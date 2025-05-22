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
    utils::{apple, facebook, jwt::JWT, validate_utils::validate_username},
};

pub struct AuthService<'a> {
    db: &'a db::Db,
    ctx: &'a Ctx,
    jwt: Arc<JWT>,
    user_repository: LocalUserDbService<'a>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AuthSignUpInput {
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
pub struct AuthSignInInput {
    #[validate(custom(function = validate_username))]
    pub username: String,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub password: String,
}

impl<'a> AuthService<'a> {
    pub fn new(db: &'a db::Db, ctx: &'a Ctx, jwt: Arc<JWT>) -> AuthService<'a> {
        AuthService {
            db: db,
            ctx: ctx,
            jwt,
            user_repository: LocalUserDbService { db: &db, ctx: &ctx },
        }
    }

    pub async fn signin(&self, input: AuthSignInInput) -> CtxResult<(String, LocalUser)> {
        input.validate()?;

        let user = self
            .user_repository
            .get(UsernameIdent(input.username.to_string()).into())
            .await?;

        let _ = AuthenticationDbService {
            db: self.db,
            ctx: self.ctx,
        }
        .authenticate(
            &self.ctx,
            user.id.clone().unwrap().to_raw(),
            AuthType::PASSWORD(Some(input.password.to_string())),
        )
        .await?;

        let token = self.jwt.encode(&user).map_err(|e| {
            self.ctx
                .to_ctx_error(AppError::AuthFailJwtInvalid { source: e })
        })?;

        Ok((token, user))
    }

    pub async fn signup(&self, input: AuthSignUpInput) -> CtxResult<String> {
        input.validate()?;

        let result = self.is_exists(input.username.clone()).await;

        if result {
            return Err(self.ctx.to_ctx_error(AppError::Generic {
                description: "The username is already used".to_string(),
            }));
        };

        let user = LocalUser {
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
            .create(user, AuthType::PASSWORD(Some(input.password)))
            .await?;

        Ok(user_id)
    }

    pub async fn sign_by_apple(
        &self,
        token: &str,
        client_id: &str,
    ) -> CtxResult<(String, LocalUser)> {
        let apple_user = apple::verify_token(token, client_id)
            .await
            .map_err(|_| self.ctx.to_ctx_error(AppError::AuthenticationFail))?;

        let auth_db_service = AuthenticationDbService {
            db: self.db,
            ctx: self.ctx,
        };

        let res_user_id = auth_db_service
            .authenticate_by_type(&self.ctx, AuthType::APPLE(apple_user.id.clone()))
            .await;

        let user_id = match res_user_id {
            Ok(user_id) => user_id,
            Err(_) => {
                let new_user = LocalUser {
                    id: None,
                    username: self
                        .build_username(Some(apple_user.email.clone()), apple_user.name.clone())
                        .await,
                    full_name: apple_user.name,
                    birth_date: None,
                    phone: None,
                    email: Some(apple_user.email),
                    bio: None,
                    social_links: None,
                    image_uri: None,
                };
                self.user_repository
                    .create(new_user, AuthType::APPLE(apple_user.id))
                    .await?
            }
        };

        let user = self
            .user_repository
            .get(IdentIdName::Id(get_string_thing(user_id)?))
            .await?;

        let token = self.jwt.encode(&user).map_err(|e| {
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

        let auth_db_service = AuthenticationDbService {
            db: self.db,
            ctx: self.ctx,
        };

        let res_user_id = auth_db_service
            .authenticate_by_type(&self.ctx, AuthType::FACEBOOK(fb_user.id.clone()))
            .await;

        let user_id = match res_user_id {
            Ok(user_id) => user_id,
            Err(_) => {
                let new_user = LocalUser {
                    id: None,
                    username: self
                        .build_username(fb_user.email, Some(fb_user.name.clone()))
                        .await,
                    full_name: Some(fb_user.name.clone()),
                    birth_date: None,
                    phone: None,
                    email: None,
                    bio: None,
                    social_links: None,
                    image_uri: None,
                };
                self.user_repository
                    .create(new_user, AuthType::FACEBOOK(fb_user.id))
                    .await?
            }
        };

        let user = self
            .user_repository
            .get(IdentIdName::Id(get_string_thing(user_id)?))
            .await?;

        let token = self.jwt.encode(&user).map_err(|e| {
            self.ctx
                .to_ctx_error(AppError::AuthFailJwtInvalid { source: e })
        })?;

        Ok((token, user))
    }

    async fn is_exists(&self, username: String) -> bool {
        self.user_repository
            .exists(UsernameIdent(username.clone()).into())
            .await
            .unwrap()
            .is_some()
    }

    async fn build_username(&self, email: Option<String>, name: Option<String>) -> String {
        if let Some(email) = email {
            let first_part = email
                .split('@')
                .next()
                .map(|s| s.to_string())
                .unwrap_or_default();

            if validate_username(&first_part).is_ok() {
                if !self.is_exists(first_part.clone()).await {
                    return first_part;
                }
            }
        };

        if let Some(name) = name {
            let name = name.trim().replace(' ', "_").to_lowercase();
            if validate_username(&name).is_ok() {
                if !self.is_exists(name.clone()).await {
                    return name;
                }
            }
        };

        Uuid::new_v4().to_string().replace("-", "_")
    }
}
