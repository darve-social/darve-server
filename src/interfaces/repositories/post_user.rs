use crate::{
    database::query_builder::SurrealQueryBuilder,
    entities::community::post_entity::PostUserStatus,
    middleware::error::AppResult,
};
use async_trait::async_trait;
use surrealdb::types::RecordId;

#[async_trait]
pub trait PostUserRepositoryInterface {
    async fn create(&self, user: RecordId, post: RecordId, status: u8) -> AppResult<()>;
    async fn update(&self, user: RecordId, post: RecordId, status: u8) -> AppResult<()>;
    fn build_upsert_query(
        &self,
        query: SurrealQueryBuilder,
        user: RecordId,
        post: RecordId,
        status: u8,
    ) -> SurrealQueryBuilder;
    async fn get(&self, user: RecordId, post: RecordId) -> AppResult<Option<PostUserStatus>>;
    async fn remove(&self, user: RecordId, posts: Vec<RecordId>) -> AppResult<()>;
}
