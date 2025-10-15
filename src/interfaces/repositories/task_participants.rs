use async_trait::async_trait;

use crate::entities::task_request_user::{TaskParticipant, TaskParticipantResult};

#[async_trait]
pub trait TaskParticipantsRepositoryInterface {
    async fn create(
        &self,
        task_id: &str,
        user_id: &str,
        status: &str,
    ) -> Result<TaskParticipant, String>;
    async fn update(
        &self,
        id: &str,
        status: &str,
        result: Option<TaskParticipantResult>,
    ) -> Result<TaskParticipant, String>;
}
