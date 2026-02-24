use async_trait::async_trait;
use surrealdb::types::RecordId;

use crate::middleware::error::AppResult;

#[async_trait]
pub trait TaskRelatesRepositoryInterface {
    async fn create(&self, task_id: &RecordId, relate_to: &RecordId) -> AppResult<()>;
}
