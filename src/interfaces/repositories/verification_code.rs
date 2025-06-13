use async_trait::async_trait;

use crate::entities::verification_code::{VerificationCode, VerificationCodeFor};

#[async_trait]
pub trait VerificationCodeRepositoryInterface {
    async fn get_by_user(
        &self,
        user_id: &str,
        use_for: VerificationCodeFor,
    ) -> Result<VerificationCode, String>;
    async fn increase_attempt(&self, code_id: &str) -> Result<(), String>;
    async fn create(
        &self,
        user_id: &str,
        code: &str,
        email: &str,
        use_for: VerificationCodeFor,
    ) -> Result<VerificationCode, String>;
    async fn delete(&self, code_id: &str) -> Result<(), String>;
}
