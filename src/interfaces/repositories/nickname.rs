use async_trait::async_trait;

use crate::{entities::nickname::Nickname, middleware::error::AppResult};

#[async_trait]
pub trait NicknamesRepositoryInterface {
    async fn upsert(&self, user_id: &str, to_user_id: &str, name: String) -> AppResult<()>;
    async fn remove(&self, user_id: &str, to_user_id: &str) -> AppResult<()>;
    async fn get_by_user(&self, user_id: &str) -> AppResult<Vec<Nickname>>;
}
