use std::sync::Arc;

use crate::{
    entities::user_auth::local_user_entity::LocalUserDbService,
    interfaces::send_email::SendEmailInterface,
    middleware::{
        error::AppError,
        utils::{db_utils::IdentIdName, string_utils::get_string_thing},
    },
    models::EmailVerificationCode,
};

use askama::Template;
use chrono::{Duration, Utc};
use surrealdb::sql::Thing;
use crate::entities::wallet::lock_transaction_entity::{LockTransactionDbService, UnlockTrigger};
use crate::entities::wallet::wallet_entity::{CurrencySymbol, WalletDbService, APP_GATEWAY_WALLET};

pub struct UserService<'a> {
    user_repository: LocalUserDbService<'a>,
    email_sender: Arc<dyn SendEmailInterface + Send + Sync>,
    email_code_ttl: Duration,
}

impl<'a> UserService<'a> {
    pub fn new(
        user_repository: LocalUserDbService<'a>,
        email_sender: Arc<dyn SendEmailInterface + Send + Sync>,
        email_code_ttl: Duration,
    ) -> Self {
        Self {
            user_repository,
            email_sender,
            email_code_ttl,
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

        let is_exists = self
            .user_repository
            .exists(IdentIdName::ColumnIdent {
                column: "email_verified".to_string(),
                val: email.to_string(),
                rec: false,
            })
            .await
            .unwrap_or_default()
            .is_some();

        if is_exists {
            return Err(AppError::Generic {
                description: "The email is already used".to_string(),
            });
        };

        let code = self.generate_verification_code();

        self.user_repository
            .create_email_verification(user.id.unwrap(), code.clone(), email.to_string())
            .await?;

        let html = EmailVerificationCode {
            code: &code,
            ttl: &self.email_code_ttl.num_minutes().to_string(),
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
        email: &str,
    ) -> Result<(), AppError> {
        let user_id_thing = get_string_thing(user_id.to_string())?;

        let user = self
            .user_repository
            .get(IdentIdName::Id(user_id_thing.clone()))
            .await?;

        let verification_data = self
            .user_repository
            .get_email_verification(user.id.clone().unwrap())
            .await?;

        if let Some(data) = verification_data {
            let is_too_many_attempts = data.failed_code_attempts >= 3;
            if is_too_many_attempts {
                return Err(AppError::Generic {
                    description: "Too many attempts. Wait and start new verification.".to_string(),
                });
            }

            let is_expired = Utc::now().signed_duration_since(data.r_created) > self.email_code_ttl;
            if is_expired {
                return Err(AppError::Generic {
                    description: "Start new verification".to_string(),
                });
            }

            if data.code != code {
                self.user_repository
                    .set_failed_verification_attempt(data.id.expect("from db"))
                    .await?;
                return Err(AppError::Generic {
                    description: "Wrong code.".to_string(),
                });
            }

            // could remove email check since we have limited tries
            if data.email == email {
                self.user_repository
                    .set_user_email(user.id.expect("from db"), email.to_string())
                    .await?;
                return Ok(());
            }
        }

        Err(AppError::Generic {
            description: "Invalid verification".to_string(),
        })
    }

    fn generate_verification_code(&self) -> String {
        use rand::Rng;
        (0..6)
            .map(|_| {
                let mut rng = rand::thread_rng();
                rng.gen_range(0..10).to_string()
            })
            .collect::<String>()
    }

}