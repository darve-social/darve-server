use std::sync::Arc;

use crate::database::surrdb_utils::record_id_key_to_string;
use crate::entities::verification_code::VerificationCodeFor;
use crate::interfaces::repositories::verification_code_ifce::VerificationCodeRepositoryInterface;
use crate::{
    entities::user_auth::{
        authentication_entity::{AuthType, AuthenticationDbService, CreateAuthInput},
        local_user_entity::LocalUserDbService,
    },
    interfaces::send_email::SendEmailInterface,
    middleware::error::{AppError, AppResult},
    utils::hash::{hash_password, verify_password},
};

use super::verification_code_service::VerificationCodeService;
use chrono::Duration;

pub struct UserService<'a, V>
where
    V: VerificationCodeRepositoryInterface + Send + Sync,
{
    user_repository: LocalUserDbService<'a>,
    auth_repository: AuthenticationDbService<'a>,
    verification_code_service: VerificationCodeService<'a, V>,
}

impl<'a, V> UserService<'a, V>
where
    V: VerificationCodeRepositoryInterface + Send + Sync,
{
    pub fn new(
        user_repository: LocalUserDbService<'a>,
        email_sender: Arc<dyn SendEmailInterface + Send + Sync>,
        code_ttl: Duration,
        auth_repository: AuthenticationDbService<'a>,
        verification_code_repository: &'a V,
    ) -> Self {
        let verification_code_service =
            VerificationCodeService::new(verification_code_repository, email_sender, code_ttl);

        Self {
            user_repository,
            auth_repository,
            verification_code_service,
        }
    }

    pub async fn start_email_verification(
        &self,
        user_id: &str,
        email: &str,
    ) -> Result<(), AppError> {
        let _ = self.user_repository.get_by_id(user_id).await?;

        let is_exists = self.user_repository.get_by_email(email).await.is_ok();

        if is_exists {
            return Err(AppError::Generic {
                description: "The email is already used".to_string(),
            });
        };

        self.verification_code_service
            .create(user_id, email, VerificationCodeFor::EmailVerification)
            .await?;

        Ok(())
    }

    pub async fn email_confirmation(
        &self,
        user_id: &str,
        code: &str,
        _email: &str,
    ) -> Result<(), AppError> {
        let user = self.user_repository.get_by_id(user_id).await?;

        let code = self
            .verification_code_service
            .get(user_id, VerificationCodeFor::EmailVerification, code)
            .await?;

        self.user_repository
            .set_user_email(user.id.unwrap(), code.email.to_string())
            .await?;

        Ok(())
    }

    pub async fn start_set_password(&self, user_id: &str) -> Result<(), AppError> {
        let (user, auth) = self
            .user_repository
            .get_by_id_with_auth(user_id, AuthType::PASSWORD)
            .await?;

        if user.email_verified.is_none() {
            return Err(AppError::Generic {
                description: "The user has not set email yet".to_string(),
            }
            .into());
        }

        if !auth.is_none() {
            return Err(AppError::Generic {
                description: "User has already set a password".to_string(),
            });
        };
        self.verification_code_service
            .create(
                user_id,
                &user.email_verified.unwrap(),
                VerificationCodeFor::SetPassword,
            )
            .await?;

        Ok(())
    }

    pub async fn start_update_password(&self, user_id: &str) -> Result<(), AppError> {
        let (user, auth) = self
            .user_repository
            .get_by_id_with_auth(user_id, AuthType::PASSWORD)
            .await?;

        if user.email_verified.is_none() {
            return Err(AppError::Generic {
                description: "The user has not set email yet".to_string(),
            }
            .into());
        }

        if auth.is_none() {
            return Err(AppError::Generic {
                description: "User hasn't set password yet".to_string(),
            });
        };

        self.verification_code_service
            .create(
                user_id,
                &user.email_verified.unwrap(),
                VerificationCodeFor::UpdatePassword,
            )
            .await?;

        Ok(())
    }

    pub async fn set_password(&self, user_id: &str, password: &str, code: &str) -> AppResult<()> {
        let (user, auth) = self
            .user_repository
            .get_by_id_with_auth(user_id, AuthType::PASSWORD)
            .await?;

        if auth.is_some() {
            return Err(AppError::Generic {
                description: "User has already set a password".to_string(),
            });
        }

        let verification_data = self
            .verification_code_service
            .get(
                &record_id_key_to_string(&user.id.as_ref().expect("exists").key),
                VerificationCodeFor::SetPassword,
                &code,
            )
            .await?;

        let (_, hash) = hash_password(password).expect("Hash password error");

        self.auth_repository
            .create(CreateAuthInput {
                local_user: user.id.as_ref().unwrap().clone(),
                token: hash,
                auth_type: AuthType::PASSWORD,
                passkey_json: None,
            })
            .await?;

        self.verification_code_service
            .delete(&verification_data.id)
            .await?;

        Ok(())
    }

    pub async fn update_password(
        &self,
        user_id: &str,
        new_pass: &str,
        old_pass: &str,
        code: &str,
    ) -> AppResult<()> {
        let (_r, auth) = self
            .user_repository
            .get_by_id_with_auth(user_id, AuthType::PASSWORD)
            .await?;

        if auth.is_none() {
            return Err(AppError::Generic {
                description: "User hasn't set password yet".to_string(),
            });
        };

        let code = self
            .verification_code_service
            .get(user_id, VerificationCodeFor::UpdatePassword, code)
            .await?;

        if !verify_password(&auth.unwrap().token, old_pass) {
            return Err(AppError::Generic {
                description: "Invalid password".to_string(),
            });
        }
        let (_, hash) = hash_password(new_pass).expect("Hash password error");

        self.auth_repository
            .update_token(user_id, AuthType::PASSWORD, hash)
            .await?;

        self.verification_code_service.delete(&code.id).await?;

        Ok(())
    }
}
