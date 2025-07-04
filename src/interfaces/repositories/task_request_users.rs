use crate::entities::task_request_user::{TaskRequestUserResult, TaskRequestUserTimeline};
use async_trait::async_trait;

#[async_trait]
pub trait TaskRequestUsersRepositoryInterface {
    async fn create(
        &self,
        task_id: &str,
        user_id: &str,
        timeline: Option<TaskRequestUserTimeline>,
    ) -> Result<String, String>;
    async fn update(
        &self,
        id: &str,
        timeline: TaskRequestUserTimeline,
        result: Option<TaskRequestUserResult>,
    ) -> Result<(), String>;
}
