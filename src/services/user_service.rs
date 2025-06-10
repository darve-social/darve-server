use std::sync::Arc;

use crate::{
    entities::user_auth::{
        authentication_entity::{AuthType, AuthenticationDbService, CreateAuthInput},
        local_user_entity::{LocalUserDbService, VerificationCodeFor},
    },
    interfaces::send_email::SendEmailInterface,
    middleware::{
        error::{AppError, AppResult},
        utils::{db_utils::IdentIdName, string_utils::get_string_thing},
    },
    models::EmailVerificationCode,
    utils::{
        generate,
        hash::{hash_password, verify_password},
    },
};

use askama::Template;
use chrono::{Duration, Utc};

pub struct UserService<'a> {
    user_repository: LocalUserDbService<'a>,
    auth_repository: AuthenticationDbService<'a>,
    email_sender: Arc<dyn SendEmailInterface + Send + Sync>,
    code_ttl: Duration,
}

impl<'a> UserService<'a> {
    pub fn new(
        user_repository: LocalUserDbService<'a>,
        email_sender: Arc<dyn SendEmailInterface + Send + Sync>,
        code_ttl: Duration,
        auth_repository: AuthenticationDbService<'a>,
    ) -> Self {
        Self {
            user_repository,
            auth_repository,
            email_sender,
            code_ttl,
        }
    }
}

impl<'a> UserService<'a> {
    pub async fn start_email_verification(
        &self,
        user_id: &str,
        email: &str,
    ) -> Result<(), AppError> {
        let user_id_thing = get_string_thing(user_id.to_string())?;

        let user = self
            .user_repository
            .get(IdentIdName::Id(user_id_thing.clone()))
            .await?;

        let is_exists = self.user_repository.get_by_email(email).await.is_ok();

        if is_exists {
            return Err(AppError::Generic {
                description: "The email is already used".to_string(),
            });
        };

        let code = generate::generate_number_code(6);

        self.user_repository
            .create_code(
                user.id.unwrap(),
                code.clone(),
                email.to_string(),
                VerificationCodeFor::EmailVerification,
            )
            .await?;

        let html = EmailVerificationCode {
            code: &code,
            ttl: &self.code_ttl.num_minutes().to_string(),
        };

        self.email_sender
            .send(
                vec![email.to_string()],
                &html.render().unwrap(),
                "Verification Email",
            )
            .await
            .map_err(|e| AppError::Generic { description: e })?;

        Ok(())
    }

    pub async fn email_confirmation(
        &self,
        user_id: &str,
        code: &str,
        _email: &str,
    ) -> Result<(), AppError> {
        let user_id_thing = get_string_thing(user_id.to_string())?;

        let user = self
            .user_repository
            .get(IdentIdName::Id(user_id_thing.clone()))
            .await?;

        let verification_data = self
            .user_repository
            .get_code(user.id.clone().unwrap(), VerificationCodeFor::EmailVerification)
            .await?;

        if let Some(data) = verification_data {
            let is_too_many_attempts = data.failed_code_attempts >= 3;
            if is_too_many_attempts {
                return Err(AppError::Generic {
                    description: "Too many attempts. Wait and start new verification.".to_string(),
                });
            }

            let is_expired = Utc::now().signed_duration_since(data.r_created) > self.code_ttl;
            if is_expired {
                return Err(AppError::Generic {
                    description: "Start new verification".to_string(),
                });
            }

            if data.code != code {
                self.user_repository.increase_code_attempt(data.id).await?;
                return Err(AppError::Generic {
                    description: "Wrong code.".to_string(),
                });
            }

            self.user_repository
                .set_user_email(user.id.unwrap(), data.email.to_string())
                .await?;
            return Ok(());
        }

        Err(AppError::Generic {
            description: "Invalid verification".to_string(),
        })
    }

    pub async fn set_password(&self, user_id: &str, password: &str) -> AppResult<()> {
        let user_thing = get_string_thing(user_id.to_string())?;

        let user = self
            .user_repository
            .get(IdentIdName::Id(user_thing.clone()))
            .await?;

        let auth = self
            .auth_repository
            .get_by_auth_type(user.id.as_ref().unwrap().to_raw(), AuthType::PASSWORD)
            .await?;

        if auth.is_some() {
            return Err(AppError::Generic {
                description: "User has already set a password".to_string(),
            });
        }

        let (_, hash) = hash_password(password).expect("Hash password error");

        self.auth_repository
            .create(CreateAuthInput {
                local_user: user_thing,
                token: hash,
                auth_type: AuthType::PASSWORD,
                passkey_json: None,
            })
            .await?;

        Ok(())
    }

    pub async fn update_password(
        &self,
        user_id: &str,
        new_pass: &str,
        old_pass: &str,
    ) -> AppResult<()> {
        let user_thing = get_string_thing(user_id.to_string())?;

        let user = self
            .user_repository
            .get(IdentIdName::Id(user_thing))
            .await?;

        let auth = self
            .auth_repository
            .get_by_auth_type(user.id.as_ref().unwrap().to_raw(), AuthType::PASSWORD)
            .await?;

        if auth.is_none() {
            return Err(AppError::Generic {
                description: "User hasn't set password yet".to_string(),
            });
        };
        if !verify_password(&auth.unwrap().token, old_pass) {
            return Err(AppError::Generic {
                description: "Invalid password".to_string(),
            });
        }
        let (_, hash) = hash_password(new_pass).expect("Hash password error");

        self.auth_repository
            .update_token(user_id.to_string(), AuthType::PASSWORD, hash)
            .await?;

        Ok(())
    }
}
