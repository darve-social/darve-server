use crate::middleware::error::AppResult;
use async_trait::async_trait;
use surrealdb::types::RecordId;

#[async_trait]
pub trait AccessRepositoryInterface {
    async fn add(&self, users: Vec<RecordId>, entities: Vec<&str>, role: String) -> AppResult<()>;
    async fn update(&self, user: RecordId, entity: &str, role: String) -> AppResult<()>;
    async fn remove_by_entity(&self, entity: &str, users: Vec<RecordId>) -> AppResult<()>;
    async fn remove_by_user(&self, user: RecordId, entities: Vec<&str>) -> AppResult<()>;
}
