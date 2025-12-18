use crate::middleware::error::AppResult;
use async_trait::async_trait;
use surrealdb::sql::Thing;

#[async_trait]
pub trait AccessRepositoryInterface {
    async fn add(&self, users: Vec<Thing>, entities: Vec<&str>, role: String) -> AppResult<()>;
    async fn update(&self, user: Thing, entity: &str, role: String) -> AppResult<()>;
    async fn remove_by_entity(&self, entity: &str, users: Vec<Thing>) -> AppResult<()>;
    async fn remove_by_user(&self, user: Thing, entities: Vec<&str>) -> AppResult<()>;
}
