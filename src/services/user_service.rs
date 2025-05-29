use std::sync::Arc;

use crate::{
    entities::user_auth::local_user_entity::{LocalUser, LocalUserDbService},
    interfaces::send_email::SendEmailInterface,
    middleware::{
        error::AppError,
        utils::{db_utils::IdentIdName, string_utils::get_string_thing},
    },
    models::EmailVerificationCode,
};

use chrono::{Duration, Utc};

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
    pub async fn email_verification(&self, user_id: &str) -> Result<(), AppError> {
        let user = self.get_user(&user_id).await?;

        let verification_data = self
            .user_repository
            .get_email_verification(user.id.clone().unwrap())
            .await?;

        if let Some(verification) = verification_data {
            let now = Utc::now();

            if now.signed_duration_since(verification.created_at) < self.email_code_ttl {
                return Err(AppError::Generic {
                    description:
                        "Verification code already sent, please wait before requesting a new one"
                            .to_string(),
                });
            }
        }

        let code = self.generate_verification_code();

        self.user_repository
            .create_email_verification(user.id.clone().unwrap(), code.clone())
            .await?;

        let html = EmailVerificationCode {
            code: &code,
            ttl: &self.email_code_ttl.num_minutes().to_string(),
        };

        self.email_sender
            .send(
                vec![user.email.unwrap()],
                &html.to_string(),
                "Verification Email",
            )
            .await
            .map_err(|e| AppError::Generic { description: e })?;

        Ok(())
    }

    pub async fn email_confirmation(&self, user_id: &str, code: &str) -> Result<(), AppError> {
        let user = self.get_user(&user_id).await?;

        let verification_data = self
            .user_repository
            .get_email_verification(user.id.clone().unwrap())
            .await?;

        if let Some(verification) = verification_data {
            let is_expired =
                Utc::now().signed_duration_since(verification.created_at) > self.email_code_ttl;
            if verification.code == code && !is_expired {
                self.user_repository
                    .verify_email(user.id.clone().unwrap())
                    .await?;
                return Ok(());
            }
        }

        Err(AppError::Generic {
            description: "Invalid verification code".to_string(),
        })
    }

    async fn get_user(&self, user_id: &str) -> Result<LocalUser, AppError> {
        let user_id_thing = get_string_thing(user_id.to_string())?;

        let user = self
            .user_repository
            .get(IdentIdName::Id(user_id_thing.clone()))
            .await?;

        if user.email_verified.unwrap_or(false) {
            return Err(AppError::Generic {
                description: "Email already verified".to_string(),
            });
        }
        if user.email.is_none() {
            return Err(AppError::Generic {
                description: "User email not found".to_string(),
            });
        }
        Ok(user)
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
