use async_trait::async_trait;
use crate::database::repository_traits::RepositoryCore;
use crate::entities::verification_code::{VerificationCodeEntity, VerificationCodeFor};

#[async_trait]
pub trait VerificationCodeRepositoryInterface: RepositoryCore {
    async fn get_by_user(
        &self,
        user_id: &str,
        use_for: VerificationCodeFor,
    ) -> Result<VerificationCodeEntity, String>;
    async fn increase_attempt(&self, code_id: &str) -> Result<(), String>;
    async fn create(
        &self,
        user_id: &str,
        code: &str,
        email: &str,
        use_for: VerificationCodeFor,
    ) -> Result<VerificationCodeEntity, String>;
    async fn delete(&self, code_id: &str) -> Result<(), String>;
}
