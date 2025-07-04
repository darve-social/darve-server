use crate::entities::task_request_user::TaskRequestUserResult;
use async_trait::async_trait;

#[async_trait]
pub trait TaskRequestUsersRepositoryInterface {
    async fn create(&self, task_id: &str, user_id: &str, status: &str) -> Result<String, String>;
    async fn update(
        &self,
        id: &str,
        status: &str,
        result: Option<TaskRequestUserResult>,
    ) -> Result<(), String>;
}
