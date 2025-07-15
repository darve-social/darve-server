use askama::Template;
use chrono::{Duration, Utc};

use crate::{
    entities::{
        user_auth::local_user_entity::LocalUser,
        verification_code::{VerificationCode, VerificationCodeFor},
    },
    interfaces::{
        repositories::verification_code::VerificationCodeRepositoryInterface,
        send_email::SendEmailInterface,
    },
    middleware::error::{AppError, AppResult},
    models::email::{EmailVerificationCode, ResetPassword},
    utils::generate,
};

pub struct VerificationCodeService<'a, V, S>
where
    V: VerificationCodeRepositoryInterface + Send + Sync,
    S: SendEmailInterface + Send + Sync,
{
    repository: &'a V,
    email_sender: &'a S,
    code_ttl: Duration,
}

impl<'a, V, S> VerificationCodeService<'a, V, S>
where
    V: VerificationCodeRepositoryInterface + Send + Sync,
    S: SendEmailInterface + Send + Sync,
{
    pub fn new(repository: &'a V, email_sender: &'a S, code_ttl: Duration) -> Self {
        Self {
            repository,
            email_sender,
            code_ttl,
        }
    }

    pub async fn delete(&self, code_id: &str) -> AppResult<()> {
        self.repository
            .delete(code_id)
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })
    }

    pub async fn create_for_password(&self, user: &LocalUser) -> AppResult<VerificationCode> {
        let code = generate::generate_number_code(6);

        let data = self
            .repository
            .create(
                &user.id.as_ref().unwrap().to_raw(),
                &code,
                &user.email_verified.as_ref().unwrap(),
                VerificationCodeFor::ResetPassword,
            )
            .await
            .map_err(|e| AppError::SurrealDb { source: e })?;

        let model = ResetPassword {
            code: &code,
            ttl: &self.code_ttl.num_minutes().to_string(),
        };

        self.email_sender
            .send(
                vec![user.email_verified.as_ref().unwrap().to_string()],
                &model.render().unwrap(),
                "Reset Password",
            )
            .await
            .map_err(|e| AppError::Generic { description: e })?;

        Ok(data)
    }

    pub async fn create_for_email(
        &self,
        user_id: &str,
        email: &str,
    ) -> AppResult<VerificationCode> {
        let code = generate::generate_number_code(6);

        let data = self
            .repository
            .create(
                user_id,
                &code,
                email,
                VerificationCodeFor::EmailVerification,
            )
            .await
            .map_err(|e| AppError::SurrealDb { source: e })?;

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

        Ok(data)
    }

    pub async fn get_verified_password_code(
        &self,
        user_id: &str,
        code: &str,
    ) -> AppResult<VerificationCode> {
        let res = self
            .get_verified_code(
                user_id,
                3,
                self.code_ttl,
                VerificationCodeFor::ResetPassword,
                code,
            )
            .await?;
        Ok(res)
    }

    pub async fn get_verified_email_code(
        &self,
        user_id: &str,
        code: &str,
    ) -> AppResult<VerificationCode> {
        let res = self
            .get_verified_code(
                user_id,
                3,
                self.code_ttl,
                VerificationCodeFor::EmailVerification,
                code,
            )
            .await?;
        Ok(res)
    }

    async fn get_verified_code(
        &self,
        user_id: &str,
        max_attempts: u8,
        code_ttl: Duration,
        use_for: VerificationCodeFor,
        code: &str,
    ) -> AppResult<VerificationCode> {
        let data = self
            .repository
            .get_by_user(user_id, use_for)
            .await
            .map_err(|e| AppError::SurrealDb { source: e })?;

        let is_too_many_attempts = data.failed_code_attempts >= max_attempts;

        if is_too_many_attempts {
            return Err(AppError::Generic {
                description: "Too many attempts. Wait and start new verification.".to_string(),
            }
            .into());
        }

        let is_expired = Utc::now().signed_duration_since(data.r_created) > code_ttl;

        if is_expired {
            return Err(AppError::Generic {
                description: "Start new verification".to_string(),
            }
            .into());
        }

        if data.code != code {
            self.repository
                .increase_attempt(&data.id)
                .await
                .map_err(|e| AppError::SurrealDb { source: e })?;

            return Err(AppError::Generic {
                description: "Wrong code.".to_string(),
            }
            .into());
        }
        Ok(data)
    }
}
