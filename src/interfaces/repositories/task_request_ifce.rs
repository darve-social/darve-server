use crate::{
    database::repository_traits::RepositoryCore,
    entities::task_request::{
        TaskForReward, TaskRequestCreate, TaskRequestStatus, TaskRequestType,
    },
    entities::task_request_user::TaskParticipantStatus,
    middleware::utils::db_utils::{Pagination, ViewFieldSelector},
};
use async_trait::async_trait;
use serde::Deserialize;
use surrealdb::method::Query;
use surrealdb::sql::Thing;

#[async_trait]
pub trait TaskRequestRepositoryInterface: RepositoryCore {
    /// Build a create query for a task request (used in transactions)
    fn build_create_query<'b>(
        &self,
        query: Query<'b, surrealdb::engine::any::Any>,
        record: &TaskRequestCreate,
    ) -> Query<'b, surrealdb::engine::any::Any>;

    /// Get task requests by post IDs
    async fn get_by_posts<T: for<'de> Deserialize<'de> + ViewFieldSelector + Send>(
        &self,
        posts: Vec<Thing>,
        user: Thing,
    ) -> Result<Vec<T>, surrealdb::Error>;

    /// Get task requests by public discussion
    async fn get_by_public_disc<T: for<'de> Deserialize<'de> + ViewFieldSelector + Send>(
        &self,
        disc_id: &str,
        user_id: &str,
        filter_by_type: Option<TaskRequestType>,
        pag: Option<Pagination>,
        is_ended: Option<bool>,
        is_acceptance_expired: Option<bool>,
    ) -> Result<Vec<T>, surrealdb::Error>;

    /// Get task requests by private discussion
    async fn get_by_private_disc<T: for<'de> Deserialize<'de> + ViewFieldSelector + Send>(
        &self,
        disc_id: &str,
        user_id: &str,
        filter_by_type: Option<TaskRequestType>,
        pag: Option<Pagination>,
        is_ended: Option<bool>,
        is_acceptance_expired: Option<bool>,
    ) -> Result<Vec<T>, surrealdb::Error>;

    /// Get task requests by user and discussion
    async fn get_by_user_and_disc<T: for<'de> Deserialize<'de> + ViewFieldSelector + Send>(
        &self,
        user_id: &str,
        disc_id: &str,
        status: Option<TaskParticipantStatus>,
    ) -> Result<Vec<T>, surrealdb::Error>;

    /// Get task requests by user participation
    async fn get_by_user<T: for<'de> Deserialize<'de> + ViewFieldSelector + Send>(
        &self,
        user: &Thing,
        status: Option<TaskParticipantStatus>,
        is_ended: Option<bool>,
        pagination: Pagination,
    ) -> Result<Vec<T>, surrealdb::Error>;

    /// Get task requests created by user
    async fn get_by_creator<T: for<'de> Deserialize<'de> + ViewFieldSelector + Send>(
        &self,
        user: Thing,
        pagination: Pagination,
    ) -> Result<Vec<T>, surrealdb::Error>;

    /// Update task request status
    async fn update_status(
        &self,
        task: Thing,
        status: TaskRequestStatus,
    ) -> Result<(), surrealdb::Error>;

    /// Get task ready for payment by ID
    async fn get_ready_for_payment_by_id(
        &self,
        id: Thing,
    ) -> Result<TaskForReward, surrealdb::Error>;

    /// Get all tasks ready for payment
    async fn get_ready_for_payment(&self) -> Result<Vec<TaskForReward>, surrealdb::Error>;
}
