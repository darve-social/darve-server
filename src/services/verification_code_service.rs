use std::sync::Arc;

use askama::Template;
use chrono::{Duration, Utc};

use crate::{
    database::surrdb_utils::record_id_key_to_string,
    entities::verification_code::{VerificationCodeEntity, VerificationCodeFor},
    interfaces::{
        repositories::verification_code_ifce::VerificationCodeRepositoryInterface,
        send_email::SendEmailInterface,
    },
    middleware::error::{AppError, AppResult},
    models::email::{EmailVerificationCode, PasswordVerificationCode},
    utils::generate,
};

pub struct VerificationCodeService<'a, V>
where
    V: VerificationCodeRepositoryInterface + Send + Sync,
{
    repository: &'a V,
    email_sender: Arc<dyn SendEmailInterface + Send + Sync>,
    code_ttl: Duration,
    max_attempts: u8,
}

impl<'a, V> VerificationCodeService<'a, V>
where
    V: VerificationCodeRepositoryInterface + Send + Sync,
{
    pub fn new(
        repository: &'a V,
        email_sender: Arc<dyn SendEmailInterface + Send + Sync>,
        code_ttl: Duration,
    ) -> Self {
        Self {
            repository,
            email_sender,
            code_ttl,
            max_attempts: 3,
        }
    }

    pub async fn delete(&self, code_id: &str) -> AppResult<()> {
        VerificationCodeRepositoryInterface::delete(self.repository, code_id)
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })
    }

    pub async fn create(
        &self,
        user_id: &str,
        verified_email: &str,
        use_for: VerificationCodeFor,
    ) -> AppResult<VerificationCodeEntity> {
        let code = generate::generate_number_code(6);
        let (html, title) = match &use_for {
            VerificationCodeFor::ResetPassword => (
                PasswordVerificationCode {
                    code: &code,
                    ttl: &self.code_ttl.num_minutes().to_string(),
                    action: "Reset",
                }
                .render()
                .unwrap(),
                "Reset Password",
            ),
            VerificationCodeFor::SetPassword => (
                PasswordVerificationCode {
                    code: &code,
                    ttl: &self.code_ttl.num_minutes().to_string(),
                    action: "Set",
                }
                .render()
                .unwrap(),
                "Set Password",
            ),
            VerificationCodeFor::UpdatePassword => (
                PasswordVerificationCode {
                    code: &code,
                    ttl: &self.code_ttl.num_minutes().to_string(),
                    action: "Update",
                }
                .render()
                .unwrap(),
                "Update Password",
            ),
            VerificationCodeFor::EmailVerification => (
                EmailVerificationCode {
                    code: &code,
                    ttl: &self.code_ttl.num_minutes().to_string(),
                }
                .render()
                .unwrap(),
                "Verification Email",
            ),
        };

        let data = VerificationCodeRepositoryInterface::create(
            self.repository,
            user_id,
            &code,
            verified_email,
            use_for,
        )
        .await?;

        self.email_sender
            .send(vec![verified_email.to_string()], &html, title)
            .await
            .map_err(|e| AppError::Generic { description: e })?;

        Ok(data)
    }

    pub async fn get(
        &self,
        user_id: &str,
        use_for: VerificationCodeFor,
        code: &str,
    ) -> AppResult<VerificationCodeEntity> {
        let data = self.repository.get_by_user(user_id, use_for).await?;
        let is_too_many_attempts = data.failed_code_attempts >= self.max_attempts;

        if is_too_many_attempts {
            return Err(AppError::Generic {
                description: "Too many attempts. Wait and start new verification.".to_string(),
            }
            .into());
        }

        let is_expired = Utc::now().signed_duration_since(data.r_created) > self.code_ttl;
        if is_expired {
            return Err(AppError::Generic {
                description: "Start new verification".to_string(),
            }
            .into());
        }

        if data.code != code {
            self.repository.increase_attempt(&record_id_key_to_string(&data.id.as_ref().unwrap().key)).await?;

            return Err(AppError::Generic {
                description: "Wrong code.".to_string(),
            }
            .into());
        }
        Ok(data)
    }
}
