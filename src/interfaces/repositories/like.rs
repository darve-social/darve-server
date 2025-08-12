use crate::middleware::error::AppResult;
use async_trait::async_trait;
use surrealdb::sql::Thing;

#[async_trait]
pub trait LikesRepositoryInterface {
    async fn like(&self, user: Thing, out: Thing, count: u16) -> AppResult<u32>;
    async fn unlike(&self, user: Thing, out: Thing) -> AppResult<u32>;
}
