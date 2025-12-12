use async_trait::async_trait;
use surrealdb::{engine::any, method::Query};

use crate::{entities::task_request_user::TaskParticipant, middleware::error::AppResult};

#[async_trait]
pub trait DeliveryResultRepositoryInterface {
    fn build_create_query<'b>(
        &self,
        query: Query<'b, any::Any>,
        task_participant_id: &str,
        post_id: &str,
        tx_id: Option<&str>,
    ) -> Query<'b, any::Any>;

    async fn get_by_post(&self, post_id: &str) -> AppResult<TaskParticipant>;
}
