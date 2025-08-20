use crate::middleware::error::AppResult;
use async_trait::async_trait;
use surrealdb::sql::Thing;

#[async_trait]
pub trait AccessRepositoryInterface {
    async fn add(&self, users: Vec<Thing>, entities: Vec<Thing>, role: String) -> AppResult<()>;
    async fn update(&self, user: Thing, entity: Thing, role: String) -> AppResult<()>;
    async fn remove_by_entity(&self, entity: Thing, users: Vec<Thing>) -> AppResult<()>;
    async fn remove_by_user(&self, user: Thing, entities: Vec<Thing>) -> AppResult<()>;
}
