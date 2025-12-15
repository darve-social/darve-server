use async_trait::async_trait;
use surrealdb::{engine::any, method::Query};

use crate::{
    entities::task_request_user::TaskParticipant, middleware::utils::db_utils::Pagination,
};

#[async_trait]
pub trait TaskParticipantsRepositoryInterface {
    fn build_create_query<'b>(
        &self,
        query: Query<'b, any::Any>,
        task_id: &str,
        user_ids: Vec<String>,
        status: &str,
    ) -> Query<'b, any::Any>;
    fn build_update_query<'b>(
        &self,
        query: Query<'b, any::Any>,
        id: &str,
        status: &str,
    ) -> Query<'b, any::Any>;
    async fn create(
        &self,
        task_id: &str,
        user_id: &str,
        status: &str,
    ) -> Result<TaskParticipant, String>;
    async fn update(&self, id: &str, status: &str) -> Result<TaskParticipant, String>;
    async fn get_by_task(
        &self,
        task_id: &str,
        pagination: Option<Pagination>,
    ) -> Result<Vec<TaskParticipant>, String>;
}
