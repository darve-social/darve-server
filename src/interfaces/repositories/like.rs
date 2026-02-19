use crate::middleware::error::AppResult;
use async_trait::async_trait;
use surrealdb::types::RecordId;

#[async_trait]
pub trait LikesRepositoryInterface {
    async fn like(&self, user: RecordId, out: RecordId, count: u16) -> AppResult<u32>;
    async fn unlike(&self, user: RecordId, out: RecordId) -> AppResult<u32>;
}
