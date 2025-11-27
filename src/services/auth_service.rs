use std::sync::Arc;

use super::verification_code_service::VerificationCodeService;
use crate::access::base::role::Role;
use crate::entities::community::discussion_entity::DiscussionDbService;
use crate::entities::user_auth::authentication_entity::Authentication;
use crate::entities::user_auth::local_user_entity::{UpdateUser, UserRole};
use crate::entities::verification_code::VerificationCodeFor;
use crate::interfaces::file_storage::FileStorageInterface;
use crate::interfaces::repositories::access::AccessRepositoryInterface;
use crate::utils;
use crate::utils::file::convert::{build_profile_file_name, convert_field_file_data};
use crate::{
    database::client::Db,
    entities::{
        community::community_entity::CommunityDbService,
        user_auth::{
            authentication_entity::{AuthType, AuthenticationDbService, CreateAuthInput},
            local_user_entity::{LocalUser, LocalUserDbService},
        },
    },
    interfaces::{
        repositories::verification_code_ifce::VerificationCodeRepositoryInterface,
        send_email::SendEmailInterface,
    },
    middleware::{
        ctx::Ctx,
        error::{AppError, CtxResult},
        utils::db_utils::{IdentIdName, UsernameIdent},
    },
    utils::{
        hash::{hash_password, verify_password},
        jwt::JWT,
        validate_utils::{validate_email_or_username, validate_username},
        verification::{apple, facebook, google},
    },
};
use axum_typed_multipart::{FieldData, TryFromMultipart};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use utils::validate_utils::validate_birth_date;
use uuid::Uuid;
use validator::{Validate, ValidateEmail};

#[derive(Debug, Validate, TryFromMultipart)]
pub struct AuthRegisterInput {
    #[form_data(limit = "unlimited")]
    pub image: Option<FieldData<NamedTempFile>>,
    #[validate(custom(function = validate_username))]
    pub username: String,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub password: String,
    #[validate(email)]
    pub email: Option<String>,
    pub bio: Option<String>,
    #[validate(custom(function = "validate_birth_date", message = "Birth date is invalid"))]
    pub birth_day: Option<DateTime<Utc>>,
    #[validate(length(min = 6, message = "Min 1 character"))]
    pub full_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct AuthLoginInput {
    #[validate(custom(function = validate_email_or_username))]
    pub username_or_email: String,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ForgotPasswordInput {
    #[validate(custom(function = validate_email_or_username, message = "Must be a valid email or username"))]
    pub email_or_username: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ResetPasswordInput {
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub password: String,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub code: String,
    #[validate(custom(function = validate_email_or_username, message = "Must be a valid email or username"))]
    pub email_or_username: String,
}

pub struct AuthService<'a, V, A>
where
    V: VerificationCodeRepositoryInterface + Send + Sync,
    A: AccessRepositoryInterface,
{
    ctx: &'a Ctx,
    jwt: &'a JWT,
    user_repository: LocalUserDbService<'a>,
    auth_repository: AuthenticationDbService<'a>,
    community_repository: CommunityDbService<'a>,
    verification_code_service: VerificationCodeService<'a, V>,
    access_repository: &'a A,
    file_storage: Arc<dyn FileStorageInterface + Send + Sync>,
}

impl<'a, V, A> AuthService<'a, V, A>
where
    V: VerificationCodeRepositoryInterface + Send + Sync,
    A: AccessRepositoryInterface,
{
    pub fn new(
        db: &'a Db,
        ctx: &'a Ctx,
        jwt: &'a JWT,
        email_sender: Arc<dyn SendEmailInterface + Send + Sync>,
        code_ttl: Duration,
        verification_code_repository: &'a V,
        access_repository: &'a A,
        file_storage: Arc<dyn FileStorageInterface + Send + Sync>,
    ) -> AuthService<'a, V, A> {
        AuthService {
            ctx,
            jwt,
            user_repository: LocalUserDbService { db: &db, ctx: &ctx },
            auth_repository: AuthenticationDbService { db: &db, ctx: &ctx },
            community_repository: CommunityDbService { db: &db, ctx: &ctx },
            verification_code_service: VerificationCodeService::new(
                verification_code_repository,
                email_sender,
                code_ttl,
            ),
            access_repository,
            file_storage,
        }
    }

    pub async fn login_password(&self, input: AuthLoginInput) -> CtxResult<(String, LocalUser)> {
        input.validate()?;

        let user = match input.username_or_email.contains("@") {
            false => {
                self.user_repository
                    .get_by_username(&input.username_or_email)
                    .await?
            }
            true => {
                self.user_repository
                    .get_by_email(&input.username_or_email)
                    .await?
            }
        };

        let auth = self
            .auth_repository
            .get_by_auth_type(user.id.as_ref().unwrap().to_raw(), AuthType::PASSWORD)
            .await?
            .ok_or(AppError::Generic {
                description: "Password not found".to_string(),
            })?;

        if !verify_password(&auth.token, &input.password) {
            return Err(AppError::Generic {
                description: "Password is not correct".to_string(),
            }
            .into());
        }
        let user_id = user.id.as_ref().unwrap().to_raw();
        let token = match user.is_otp_enabled {
            true => self
                .jwt
                .create_by_otp(&user_id)
                .map_err(|e| AppError::AuthFailJwtInvalid {
                    source: e.to_string(),
                })?,
            false => self.build_jwt_token(&user_id)?,
        };

        Ok((token, user))
    }

    pub async fn register_password(
        &self,
        input: AuthRegisterInput,
        role: Option<UserRole>,
    ) -> CtxResult<(String, LocalUser)> {
        input.validate()?;
        if self.is_exists_by_username(input.username.clone()).await {
            return Err(self.ctx.to_ctx_error(AppError::Generic {
                description: "The username is already used".to_string(),
            }));
        };

        if self.is_exists_by_email(&input.email).await {
            return Err(self.ctx.to_ctx_error(AppError::Generic {
                description: "The email is already used".to_string(),
            }));
        };

        let mut user = LocalUser::default(input.username);
        user.role = role.unwrap_or(UserRole::User);
        user.full_name = input.full_name;
        user.bio = input.bio;
        user.birth_date = input.birth_day.map(|d| d.date_naive().to_string());

        let uploaded_image = match input.image {
            Some(file) => Some(convert_field_file_data(file)?),
            None => None,
        };

        let (_, hash) = hash_password(&input.password).expect("Hash password error");
        let (token, mut user) = self.register(user, AuthType::PASSWORD, &hash).await?;

        if let Some(email) = input.email {
            let _ = self
                .verification_code_service
                .create(
                    user.id.as_ref().unwrap().id.to_raw().as_str(),
                    email.as_str(),
                    VerificationCodeFor::EmailVerification,
                )
                .await;
        }

        if let Some(data) = uploaded_image {
            let path = user.id.clone().unwrap().to_raw().replace(":", "_");
            let file_name = build_profile_file_name(&data.extension);
            let image_url = self
                .file_storage
                .upload(
                    data.data,
                    Some(&path),
                    &file_name,
                    data.content_type.as_deref(),
                )
                .await
                .map_err(|e| AppError::Generic {
                    description: e.to_string(),
                })?;

            self.user_repository
                .update(
                    &user.id.as_ref().unwrap().id.to_raw().as_str(),
                    UpdateUser {
                        bio: None,
                        birth_date: None,
                        full_name: None,
                        image_uri: Some(Some(image_url.clone())),
                        is_otp_enabled: None,
                        otp_secret: None,
                        phone: None,
                        social_links: None,
                        username: None,
                    },
                )
                .await?;

            user.image_uri = Some(image_url)
        }
        Ok((token, user))
    }

    pub async fn register_login_by_apple(
        &self,
        token: &str,
        client_id: &str,
    ) -> CtxResult<(String, LocalUser, bool)> {
        let apple_user = apple::verify_token(token, client_id)
            .await
            .map_err(|_| self.ctx.to_ctx_error(AppError::AuthenticationFail))?;

        let res = self
            .get_user_id_by_social_auth(
                AuthType::APPLE,
                apple_user.id.clone(),
                apple_user.email.clone(),
            )
            .await;

        match res {
            Ok((user, pass_auth)) => {
                let token = self.build_jwt_token(&user.id.as_ref().unwrap().to_raw())?;
                Ok((token, user, pass_auth.is_some()))
            }
            Err(err) => match err.error {
                AppError::EntityFailIdNotFound { .. } => {
                    let mut new_user = LocalUser::default(
                        self.build_username(apple_user.email.clone(), apple_user.name.clone())
                            .await,
                    );

                    new_user.full_name = apple_user.name;
                    new_user.email_verified = apple_user.email;
                    let (token, user) = self
                        .register(new_user, AuthType::APPLE, &apple_user.id)
                        .await?;

                    Ok((token, user, false))
                }
                _ => Err(err),
            },
        }
    }

    pub async fn sign_by_facebook(&self, token: &str) -> CtxResult<(String, LocalUser, bool)> {
        let fb_user = facebook::verify_token(token)
            .await
            .map_err(|_| self.ctx.to_ctx_error(AppError::AuthenticationFail))?;

        let res = self
            .get_user_id_by_social_auth(
                AuthType::FACEBOOK,
                fb_user.id.clone(),
                fb_user.email.clone(),
            )
            .await;

        match res {
            Ok((user, pass_auth)) => {
                let token = self.build_jwt_token(&user.id.as_ref().unwrap().to_raw())?;
                Ok((token, user, pass_auth.is_some()))
            }
            Err(err) => match err.error {
                AppError::EntityFailIdNotFound { .. } => {
                    let mut new_user = LocalUser::default(
                        self.build_username(fb_user.email, Some(fb_user.name.clone()))
                            .await,
                    );

                    new_user.full_name = Some(fb_user.name.clone());
                    let (token, user) = self
                        .register(new_user, AuthType::FACEBOOK, &fb_user.id)
                        .await?;
                    Ok((token, user, false))
                }
                _ => Err(err),
            },
        }
    }

    pub async fn sign_by_google(
        &self,
        token: &str,
        google_client_ids: &Vec<&str>,
    ) -> CtxResult<(String, LocalUser, bool)> {
        let google_user = google::verify_token(token, google_client_ids)
            .await
            .map_err(|_| self.ctx.to_ctx_error(AppError::AuthenticationFail))?;

        let res = self
            .get_user_id_by_social_auth(
                AuthType::GOOGLE,
                google_user.sub.clone(),
                google_user.email.clone(),
            )
            .await;

        match res {
            Ok((user, pass_auth)) => {
                let token = self.build_jwt_token(&user.id.as_ref().unwrap().to_raw())?;
                Ok((token, user, pass_auth.is_some()))
            }
            Err(err) => match err.error {
                AppError::EntityFailIdNotFound { .. } => {
                    let mut new_user = LocalUser::default(
                        self.build_username(google_user.email.clone(), google_user.name.clone())
                            .await,
                    );
                    new_user.full_name = google_user.name;
                    new_user.email_verified = google_user.email;
                    new_user.image_uri = google_user.picture;
                    let (token, user) = self
                        .register(new_user, AuthType::GOOGLE, &google_user.sub)
                        .await?;
                    Ok((token, user, false))
                }
                _ => Err(err),
            },
        }
    }

    pub async fn reset_password(&self, input: ResetPasswordInput) -> CtxResult<()> {
        input.validate()?;

        let user = if input.email_or_username.validate_email() {
            self.user_repository
                .get_by_email(&input.email_or_username)
                .await?
        } else {
            self.user_repository
                .get_by_username(&input.email_or_username)
                .await?
        };

        let verification_data = self
            .verification_code_service
            .get(
                &user.id.as_ref().expect("exists").id.to_raw(),
                VerificationCodeFor::ResetPassword,
                &input.code,
            )
            .await?;

        let (_, hash) = hash_password(&input.password).expect("Hash password error");

        self.auth_repository
            .update_token(
                &user.id.as_ref().unwrap().id.to_raw(),
                AuthType::PASSWORD,
                hash,
            )
            .await?;

        self.verification_code_service
            .delete(&verification_data.id)
            .await?;

        Ok(())
    }

    pub async fn forgot_password(&self, data: ForgotPasswordInput) -> CtxResult<()> {
        data.validate()?;

        let (user, auth) = if data.email_or_username.validate_email() {
            self.user_repository
                .get_by_email_with_auth(&data.email_or_username, AuthType::PASSWORD)
                .await?
        } else {
            self.user_repository
                .get_by_username_with_auth(&data.email_or_username, AuthType::PASSWORD)
                .await?
        };

        if user.email_verified.is_none() {
            return Err(AppError::Generic {
                description: "The user has not set email yet".to_string(),
            }
            .into());
        }

        if auth.is_none() {
            return Err(AppError::Generic {
                description: "User has not set password yet".to_string(),
            }
            .into());
        }

        let _ = self
            .verification_code_service
            .create(
                &user.id.expect("exists").id.to_raw(),
                &user.email_verified.expect("email exists"),
                VerificationCodeFor::ResetPassword,
            )
            .await?;

        Ok(())
    }

    async fn get_user_id_by_social_auth(
        &self,
        auth: AuthType,
        token: String,
        email: Option<String>,
    ) -> CtxResult<(LocalUser, Option<Authentication>)> {
        let auth = self.auth_repository.get_by_token(auth, token).await?;

        if auth.is_some() {
            return self
                .user_repository
                .get_by_id_with_auth(&auth.unwrap().local_user.id.to_raw(), AuthType::PASSWORD)
                .await;
        }

        match email {
            Some(val) => {
                self.user_repository
                    .get_by_email_with_auth(&val, AuthType::PASSWORD)
                    .await
            }
            None => Err(self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
                ident: "".to_string(),
            })),
        }
    }

    async fn is_exists_by_username(&self, username: String) -> bool {
        self.user_repository
            .exists(UsernameIdent(username.clone()).into())
            .await
            .unwrap()
            .is_some()
    }

    async fn is_exists_by_email(&self, email: &Option<String>) -> bool {
        if email.is_none() {
            return false;
        }

        let ident = IdentIdName::ColumnIdent {
            column: "email_verified".to_string(),
            val: email.clone().unwrap(),
            rec: false,
        };

        self.user_repository.exists(ident).await.unwrap().is_some()
    }

    async fn build_username(&self, email: Option<String>, name: Option<String>) -> String {
        if let Some(email) = email {
            let first_part = email
                .split('@')
                .next()
                .map(|s| s.to_string())
                .unwrap_or_default();

            if validate_username(&first_part).is_ok() {
                if !self.is_exists_by_username(first_part.clone()).await {
                    return first_part;
                }
            }
        };

        if let Some(name) = name {
            let name = name.trim().replace(' ', "_").to_lowercase();
            if validate_username(&name).is_ok() {
                if !self.is_exists_by_username(name.clone()).await {
                    return name;
                }
            }
        };

        Uuid::new_v4().to_string().replace("-", "_")
    }

    fn build_jwt_token(&self, user_id: &String) -> CtxResult<String> {
        Ok(self.jwt.create_by_login(user_id).map_err(|e| {
            self.ctx
                .to_ctx_error(AppError::AuthFailJwtInvalid { source: e })
        })?)
    }

    async fn register(
        &self,
        data: LocalUser,
        auth_type: AuthType,
        token: &str,
    ) -> CtxResult<(String, LocalUser)> {
        let user = self.user_repository.create(data).await?;
        let _ = self
            .auth_repository
            .create(CreateAuthInput {
                local_user: user.id.as_ref().unwrap().clone(),
                token: token.to_string(),
                auth_type,
                passkey_json: None,
            })
            .await?;
        let token = self.build_jwt_token(&user.id.as_ref().unwrap().to_raw())?;
        let disc_id = DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());
        let _ = self
            .community_repository
            .create_profile(disc_id.clone(), user.id.as_ref().unwrap().clone())
            .await?;

        let _ = self
            .access_repository
            .add(
                vec![user.id.as_ref().unwrap().clone()],
                [disc_id].to_vec(),
                Role::Owner.to_string(),
            )
            .await?;
        Ok((token, user))
    }
}
