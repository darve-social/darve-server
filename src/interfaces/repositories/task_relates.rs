use async_trait::async_trait;
use surrealdb::sql::Thing;

use crate::middleware::error::AppResult;

#[async_trait]
pub trait TaskRelatesRepositoryInterface {
    async fn create(&self, task_id: &Thing, relate_to: &Thing) -> AppResult<()>;
}
