use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

use crate::{
    entities::user_auth::{
        authentication_entity::{AuthType, AuthenticationDbService},
        local_user_entity::{LocalUser, LocalUserDbService},
    },
    middleware::{
        ctx::Ctx,
        db,
        error::{AppError, CtxResult},
        utils::{
            db_utils::{IdentIdName, UsernameIdent},
            string_utils::get_string_thing,
        },
    },
    utils::{
        jwt::JWT,
        validate_utils::validate_username,
        verification::{apple, facebook, google},
    },
};

pub struct AuthService<'a> {
    ctx: &'a Ctx,
    jwt: Arc<JWT>,
    user_repository: LocalUserDbService<'a>,
    auth_repository: AuthenticationDbService<'a>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AuthRegisterInput {
    #[validate(custom(function = validate_username))]
    pub username: String,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub password: String,
    #[validate(email)]
    pub email: Option<String>,
    pub bio: Option<String>,
    pub birth_day: Option<DateTime<Utc>>,
    #[validate(length(min = 6, message = "Min 1 character"))]
    pub full_name: Option<String>,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub image_uri: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AuthLoginInput {
    #[validate(custom(function = validate_username))]
    pub username: String,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub password: String,
}

impl<'a> AuthService<'a> {
    pub fn new(db: &'a db::Db, ctx: &'a Ctx, jwt: Arc<JWT>) -> AuthService<'a> {
        AuthService {
            ctx,
            jwt,
            user_repository: LocalUserDbService { db: &db, ctx: &ctx },
            auth_repository: AuthenticationDbService { db: &db, ctx: &ctx },
        }
    }

    pub async fn login_password(&self, input: AuthLoginInput) -> CtxResult<(String, LocalUser)> {
        input.validate()?;

        let user = self
            .user_repository
            .get(UsernameIdent(input.username.to_string()).into())
            .await?;

        self.auth_repository
            .authenticate(
                &self.ctx,
                AuthType::PASSWORD(Some(input.password.to_string()), user.id.clone()),
            )
            .await?;

        Ok((
            self.build_jwt_token(&user.id.as_ref().unwrap().to_raw())
                .await?,
            user,
        ))
    }

    pub async fn register_password(&self, input: AuthRegisterInput) -> CtxResult<(String, LocalUser)> {
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

        let mut user = LocalUser {
            id: None,
            username: input.username,
            full_name: input.full_name,
            phone: None,
            email: input.email,
            bio: input.bio,
            social_links: None,
            image_uri: input.image_uri,
            birth_date: input.birth_day,
        };

        let user_id = self
            .user_repository
            .create(user.clone(), AuthType::PASSWORD(Some(input.password), None))
            .await?;

        user.id = Some(get_string_thing(user_id.clone())?);

        Ok((
            self.build_jwt_token(&user.id.as_ref().unwrap().to_raw())
                .await?,
            user,
        ))
    }

    pub async fn register_login_by_apple(
        &self,
        token: &str,
        client_id: &str,
    ) -> CtxResult<(String, LocalUser)> {
        let apple_user = apple::verify_token(token, client_id)
            .await
            .map_err(|_| self.ctx.to_ctx_error(AppError::AuthenticationFail))?;

        let auth = AuthType::APPLE(apple_user.id);

        let res_user_id = self
            .get_user_id_by_social_auth(auth.clone(), Some(apple_user.email.clone()))
            .await;

        // TODO -create reuse same method- register_user(...) that creates and returns jwt, replace replecated code in other functions
        let user_id = match res_user_id {
            Ok(user_id) => user_id,
            Err(_) => {
                let username = self
                    .build_username(Some(apple_user.email.clone()), apple_user.name.clone())
                    .await;
                let new_user = LocalUser {
                    id: None,
                    username,
                    full_name: apple_user.name,
                    birth_date: None,
                    phone: None,
                    email: Some(apple_user.email),
                    bio: None,
                    social_links: None,
                    image_uri: None,
                };
                self.user_repository.create(new_user, auth).await?
            }
        };

        let user = self
            .user_repository
            .get(IdentIdName::Id(get_string_thing(user_id)?))
            .await?;

        let token = self
            .jwt
            .encode(&user.id.clone().unwrap().to_raw())
            .map_err(|e| {
                self.ctx
                    .to_ctx_error(AppError::AuthFailJwtInvalid { source: e })
            })?;

        Ok((token, user))
    }

    pub async fn sign_by_facebook(&self, token: &str) -> CtxResult<(String, LocalUser)> {
        let fb_user = match facebook::verify_token(token).await {
            Some(v) => v,
            None => return Err(self.ctx.to_ctx_error(AppError::AuthenticationFail)),
        };

        let auth: AuthType = AuthType::FACEBOOK(fb_user.id.clone());
        let res_user_id = self
            .get_user_id_by_social_auth(auth.clone(), fb_user.email.clone())
            .await;

        let user_id = match res_user_id {
            Ok(user_id) => user_id,
            Err(_) => {
                let username = self
                    .build_username(fb_user.email, Some(fb_user.name.clone()))
                    .await;
                let new_user = LocalUser {
                    id: None,
                    username,
                    full_name: Some(fb_user.name.clone()),
                    birth_date: None,
                    phone: None,
                    email: None,
                    bio: None,
                    social_links: None,
                    image_uri: None,
                };
                self.user_repository.create(new_user, auth).await?
            }
        };

        let user = self
            .user_repository
            .get(IdentIdName::Id(get_string_thing(user_id)?))
            .await?;

        let token = self
            .jwt
            .encode(&user.id.clone().unwrap().to_raw())
            .map_err(|e| {
                self.ctx
                    .to_ctx_error(AppError::AuthFailJwtInvalid { source: e })
            })?;

        Ok((token, user))
    }

    pub async fn sign_by_google(
        &self,
        token: &str,
        google_client_id: &str,
    ) -> CtxResult<(String, LocalUser)> {
        let google_user = google::verify_token(token, google_client_id)
            .await
            .map_err(|_| self.ctx.to_ctx_error(AppError::AuthenticationFail))?;

        let auth: AuthType = AuthType::GOOGLE(google_user.sub.clone());
        let res_user_id = self
            .get_user_id_by_social_auth(auth.clone(), Some(google_user.email.clone()))
            .await;

        let user_id = match res_user_id {
            Ok(user_id) => user_id,
            Err(_) => {
                let username = self
                    .build_username(Some(google_user.email.clone()), google_user.name.clone())
                    .await;
                let new_user = LocalUser {
                    id: None,
                    username,
                    full_name: google_user.name,
                    birth_date: None,
                    phone: None,
                    email: Some(google_user.email),
                    bio: None,
                    social_links: None,
                    image_uri: google_user.picture,
                };
                self.user_repository.create(new_user, auth).await?
            }
        };

        let user = self
            .user_repository
            .get(IdentIdName::Id(get_string_thing(user_id)?))
            .await?;

        Ok((
            self.build_jwt_token(&user.id.as_ref().unwrap().to_raw())
                .await?,
            user,
        ))
    }

    async fn get_user_id_by_social_auth(
        &self,
        auth: AuthType,
        email: Option<String>,
    ) -> CtxResult<String> {
        let res_user_id = self
            .auth_repository
            .authenticate(&self.ctx, auth.clone())
            .await;

        if res_user_id.is_ok() {
            return res_user_id;
        }
        match email {
            Some(val) => {
                let user = self
                    .user_repository
                    .get(IdentIdName::ColumnIdent {
                        column: "email".to_string(),
                        val,
                        rec: false,
                    })
                    .await?;
                // TODO -after verification- check if user.email_verified
                Ok(user.id.unwrap().to_raw())
            }
            None => Err(self.ctx.to_ctx_error(AppError::AuthenticationFail {})),
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
            column: "email".to_string(),
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

    async fn build_jwt_token(&self, user_id: &String) -> CtxResult<String> {
        Ok(self.jwt.encode(user_id).map_err(|e| {
            self.ctx
                .to_ctx_error(AppError::AuthFailJwtInvalid { source: e })
        })?)
    }
}
