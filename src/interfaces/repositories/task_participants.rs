use async_trait::async_trait;

use crate::{
    database::query_builder::SurrealQueryBuilder,
    entities::task_request_user::{TaskParticipant, TaskParticipantResult},
    middleware::utils::db_utils::Pagination,
};

#[async_trait]
pub trait TaskParticipantsRepositoryInterface {
    fn build_create_query(
        &self,
        query: SurrealQueryBuilder,
        task_id: &str,
        user_ids: Vec<String>,
        status: &str,
    ) -> SurrealQueryBuilder;
    fn build_update_query(
        &self,
        query: SurrealQueryBuilder,
        id: &str,
        status: &str,
        result: Option<&TaskParticipantResult>,
    ) -> SurrealQueryBuilder;
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
        result: Option<&TaskParticipantResult>,
    ) -> Result<TaskParticipant, String>;
    async fn get_by_task(
        &self,
        task_id: &str,
        pagination: Option<Pagination>,
    ) -> Result<Vec<TaskParticipant>, String>;
}
