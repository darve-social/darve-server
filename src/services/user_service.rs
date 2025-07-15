use crate::interfaces::repositories::verification_code_ifce::VerificationCodeRepositoryInterface;
use crate::{
    entities::user_auth::{
        authentication_entity::{AuthType, AuthenticationDbService, CreateAuthInput},
        local_user_entity::LocalUserDbService,
    },
    interfaces::send_email::SendEmailInterface,
    middleware::{
        error::{AppError, AppResult},
        utils::{db_utils::IdentIdName, string_utils::get_string_thing},
    },
    utils::hash::{hash_password, verify_password},
};

use super::verification_code_service::VerificationCodeService;
use chrono::Duration;

pub struct UserService<'a, V, S>
where
    V: VerificationCodeRepositoryInterface + Send + Sync,
    S: SendEmailInterface + Send + Sync,
{
    user_repository: LocalUserDbService<'a>,
    auth_repository: AuthenticationDbService<'a>,
    verification_code_service: VerificationCodeService<'a, V, S>,
}

impl<'a, V, S> UserService<'a, V, S>
where
    V: VerificationCodeRepositoryInterface + Send + Sync,
    S: SendEmailInterface + Send + Sync,
{
    pub fn new(
        user_repository: LocalUserDbService<'a>,
        email_sender: &'a S,
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

        let _ = self
            .user_repository
            .get_by_id(user_id)
            .await?;

        let is_exists = self.user_repository.get_by_email(email).await.is_ok();

        if is_exists {
            return Err(AppError::Generic {
                description: "The email is already used".to_string(),
            });
        };

        self
            .verification_code_service
            .create_for_email(user_id, email)
            .await?;

        Ok(())
    }

    pub async fn email_confirmation(
        &self,
        user_id: &str,
        code: &str,
        _email: &str,
    ) -> Result<(), AppError> {

        let user = self
            .user_repository
            .get_by_id(user_id)
            .await?;

        let code = self
            .verification_code_service
            .get_verified_email_code(user_id, code)
            .await?;

        self.user_repository
            .set_user_email(user.id.unwrap(), code.email.to_string())
            .await?;

        Ok(())
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
