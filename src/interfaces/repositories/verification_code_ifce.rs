use crate::{
    database::repository_traits::RepositoryCore,
    entities::verification_code::{VerificationCodeEntity, VerificationCodeFor},
};
use async_trait::async_trait;

#[async_trait]
pub trait VerificationCodeRepositoryInterface: RepositoryCore {
    async fn get_by_user(
        &self,
        user_id: &str,
        use_for: VerificationCodeFor,
    ) -> Result<VerificationCodeEntity, surrealdb::Error>;
    async fn increase_attempt(&self, code_id: &str) -> Result<(), surrealdb::Error>;
    async fn create(
        &self,
        user_id: &str,
        code: &str,
        email: &str,
        use_for: VerificationCodeFor,
    ) -> Result<VerificationCodeEntity, surrealdb::Error>;
    async fn delete(&self, code_id: &str) -> Result<(), surrealdb::Error>;
}
