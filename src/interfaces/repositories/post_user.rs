use crate::{entities::community::post_entity::PostUserStatus, middleware::error::AppResult};
use async_trait::async_trait;
use surrealdb::{engine::any, method::Query, sql::Thing};

#[async_trait]
pub trait PostUserRepositoryInterface {
    async fn create(&self, user: Thing, post: Thing, status: u8) -> AppResult<()>;
    async fn update(&self, user: Thing, post: Thing, status: u8) -> AppResult<()>;
    fn build_upsert_query<'b>(
        &self,
        query: Query<'b, any::Any>,
        user: Thing,
        post: Thing,
        status: u8,
    ) -> Query<'b, any::Any>;
    async fn get(&self, user: Thing, post: Thing) -> AppResult<Option<PostUserStatus>>;
    async fn remove(&self, user: Thing, posts: Vec<Thing>) -> AppResult<()>;
}
