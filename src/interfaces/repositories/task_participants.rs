use async_trait::async_trait;
use surrealdb::{engine::any, method::Query};

use crate::entities::task_request_user::{TaskParticipant, TaskParticipantResult};

#[async_trait]
pub trait TaskParticipantsRepositoryInterface {
    fn build_create_query<'b>(
        &self,
        query: Query<'b, any::Any>,
        task_id: &str,
        user_id: &str,
        status: &str,
    ) -> Query<'b, any::Any>;
    fn build_update_query<'b>(
        &self,
        query: Query<'b, any::Any>,
        id: &str,
        status: &str,
        result: Option<TaskParticipantResult>,
    ) -> Query<'b, any::Any>;
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
