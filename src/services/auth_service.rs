use super::verification_code_service::VerificationCodeService;
use crate::entities::verification_code::VerificationCodeFor;
use crate::utils;
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
        utils::{
            db_utils::{IdentIdName, UsernameIdent},
            string_utils::get_string_thing,
        },
    },
    utils::{
        hash::{hash_password, verify_password},
        jwt::JWT,
        validate_utils::{validate_email_or_username, validate_username},
        verification::{apple, facebook, google},
    },
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use utils::validate_utils::validate_birth_date;
use uuid::Uuid;
use validator::{Validate, ValidateEmail};

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct AuthRegisterInput {
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
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub image_uri: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct AuthLoginInput {
    #[validate(custom(function = validate_username))]
    pub username: String,
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

pub struct AuthService<'a, V, S>
where
    V: VerificationCodeRepositoryInterface + Send + Sync,
    S: SendEmailInterface + Send + Sync,
{
    ctx: &'a Ctx,
    jwt: &'a JWT,
    user_repository: LocalUserDbService<'a>,
    auth_repository: AuthenticationDbService<'a>,
    community_repository: CommunityDbService<'a>,
    verification_code_service: VerificationCodeService<'a, V, S>,
}

impl<'a, V, S> AuthService<'a, V, S>
where
    V: VerificationCodeRepositoryInterface + Send + Sync,
    S: SendEmailInterface + Send + Sync,
{
    pub fn new(
        db: &'a Db,
        ctx: &'a Ctx,
        jwt: &'a JWT,
        email_sender: &'a S,
        code_ttl: Duration,
        verification_code_repository: &'a V,
    ) -> AuthService<'a, V, S> {
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
        }
    }

    pub async fn login_password(&self, input: AuthLoginInput) -> CtxResult<(String, LocalUser)> {
        input.validate()?;

        let user = self
            .user_repository
            .get_by_username(&input.username)
            .await?;

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
        user.full_name = input.full_name;
        user.bio = input.bio;
        user.image_uri = input.image_uri;
        user.birth_date = input.birth_day.map(|d| d.date_naive().to_string());

        let (_, hash) = hash_password(&input.password).expect("Hash password error");
        self.register(user, AuthType::PASSWORD, &hash).await
    }

    pub async fn register_login_by_apple(
        &self,
        token: &str,
        client_id: &str,
    ) -> CtxResult<(String, LocalUser)> {
        let apple_user = apple::verify_token(token, client_id)
            .await
            .map_err(|_| self.ctx.to_ctx_error(AppError::AuthenticationFail))?;

        let res_user_id = self
            .get_user_id_by_social_auth(
                AuthType::APPLE,
                apple_user.id.clone(),
                apple_user.email.clone(),
            )
            .await;

        match res_user_id {
            Ok(user_id) => {
                let user = self
                    .user_repository
                    .get(IdentIdName::Id(get_string_thing(user_id)?))
                    .await?;

                let token = self.build_jwt_token(&user.id.as_ref().unwrap().to_raw())?;
                Ok((token, user))
            }
            Err(err) => match err.error {
                AppError::EntityFailIdNotFound { .. } => {
                    let mut new_user = LocalUser::default(
                        self.build_username(apple_user.email.clone(), apple_user.name.clone())
                            .await,
                    );

                    new_user.full_name = apple_user.name;
                    new_user.email_verified = apple_user.email;
                    return self
                        .register(new_user, AuthType::APPLE, &apple_user.id)
                        .await;
                }
                _ => Err(err),
            },
        }
    }

    pub async fn sign_by_facebook(&self, token: &str) -> CtxResult<(String, LocalUser)> {
        let fb_user = facebook::verify_token(token)
            .await
            .map_err(|_| self.ctx.to_ctx_error(AppError::AuthenticationFail))?;

        let res_user_id = self
            .get_user_id_by_social_auth(
                AuthType::FACEBOOK,
                fb_user.id.clone(),
                fb_user.email.clone(),
            )
            .await;

        match res_user_id {
            Ok(user_id) => {
                let user = self
                    .user_repository
                    .get(IdentIdName::Id(get_string_thing(user_id)?))
                    .await?;

                let token = self.build_jwt_token(&user.id.as_ref().unwrap().to_raw())?;
                Ok((token, user))
            }
            Err(err) => match err.error {
                AppError::EntityFailIdNotFound { .. } => {
                    let mut new_user = LocalUser::default(
                        self.build_username(fb_user.email, Some(fb_user.name.clone()))
                            .await,
                    );

                    new_user.full_name = Some(fb_user.name.clone());
                    return self
                        .register(new_user, AuthType::FACEBOOK, &fb_user.id)
                        .await;
                }
                _ => Err(err),
            },
        }
    }

    pub async fn sign_by_google(
        &self,
        token: &str,
        google_client_ids: &Vec<&str>,
    ) -> CtxResult<(String, LocalUser)> {
        let google_user = google::verify_token(token, google_client_ids)
            .await
            .map_err(|_| self.ctx.to_ctx_error(AppError::AuthenticationFail))?;

        let res_user_id = self
            .get_user_id_by_social_auth(
                AuthType::GOOGLE,
                google_user.sub.clone(),
                google_user.email.clone(),
            )
            .await;

        match res_user_id {
            Ok(user_id) => {
                let user = self
                    .user_repository
                    .get(IdentIdName::Id(get_string_thing(user_id)?))
                    .await?;

                let token = self.build_jwt_token(&user.id.as_ref().unwrap().to_raw())?;
                Ok((token, user))
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
                    return self
                        .register(new_user, AuthType::GOOGLE, &google_user.sub)
                        .await;
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

        let user = if data.email_or_username.validate_email() {
            self.user_repository
                .get_by_email(&data.email_or_username)
                .await?
        } else {
            self.user_repository
                .get_by_username(&data.email_or_username)
                .await?
        };

        if user.email_verified.is_none() {
            return Err(AppError::Generic {
                description: "The user has not set email yet".to_string(),
            }
            .into());
        }

        let auth = self
            .auth_repository
            .get_by_auth_type(user.id.as_ref().unwrap().to_raw(), AuthType::PASSWORD)
            .await?;

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
    ) -> CtxResult<String> {
        let auth = self.auth_repository.get_by_token(auth, token).await?;

        if auth.is_some() {
            return Ok(auth.unwrap().local_user.to_raw());
        }

        match email {
            Some(val) => {
                let user = self
                    .user_repository
                    .get(IdentIdName::ColumnIdent {
                        column: "email_verified".to_string(),
                        val,
                        rec: false,
                    })
                    .await?;
                Ok(user.id.unwrap().to_raw())
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

        self.community_repository
            .create_profile(user.id.as_ref().unwrap().clone())
            .await?;

        Ok((token, user))
    }
}
