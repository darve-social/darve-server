use crate::{
    database::{query_builder::SurrealQueryBuilder, repository_traits::RepositoryCore},
    entities::{
        task_request::{TaskForReward, TaskRequestCreate, TaskRequestStatus, TaskRequestType},
        task_request_user::TaskParticipantStatus,
    },
    middleware::{
        error::AppResult,
        utils::db_utils::{Pagination, ViewFieldSelector},
    },
};
use async_trait::async_trait;
use serde::Deserialize;
use surrealdb::types::{RecordId, SurrealValue};

#[async_trait]
pub trait TaskRequestRepositoryInterface: RepositoryCore {
    /// Build a create query for a task request (used in transactions)
    fn build_create_query(
        &self,
        query: SurrealQueryBuilder,
        record: &TaskRequestCreate,
    ) -> SurrealQueryBuilder;

    /// Get task requests by post IDs
    async fn get_by_posts<T: for<'de> Deserialize<'de> + SurrealValue + ViewFieldSelector + Send>(
        &self,
        posts: Vec<RecordId>,
        user: RecordId,
    ) -> Result<Vec<T>, surrealdb::Error>;

    /// Get task requests by public discussion
    async fn get_by_public_disc<T: for<'de> Deserialize<'de> + SurrealValue + ViewFieldSelector + Send>(
        &self,
        disc_id: &str,
        user_id: &str,
        filter_by_type: Option<TaskRequestType>,
        pag: Option<Pagination>,
        is_ended: Option<bool>,
        is_acceptance_expired: Option<bool>,
    ) -> Result<Vec<T>, surrealdb::Error>;

    /// Get task requests by private discussion
    async fn get_by_private_disc<T: for<'de> Deserialize<'de> + SurrealValue + ViewFieldSelector + Send>(
        &self,
        disc_id: &str,
        user_id: &str,
        filter_by_type: Option<TaskRequestType>,
        pag: Option<Pagination>,
        is_ended: Option<bool>,
        is_acceptance_expired: Option<bool>,
    ) -> Result<Vec<T>, surrealdb::Error>;

    /// Get task requests by user and discussion
    async fn get_by_user_and_disc<T: for<'de> Deserialize<'de> + SurrealValue + ViewFieldSelector + Send>(
        &self,
        user_id: &str,
        disc_id: &str,
        status: Option<TaskParticipantStatus>,
    ) -> Result<Vec<T>, surrealdb::Error>;

    /// Get task requests by user participation
    async fn get_by_user<T: for<'de> Deserialize<'de> + SurrealValue + ViewFieldSelector + Send>(
        &self,
        user: &RecordId,
        status: Option<TaskParticipantStatus>,
        is_ended: Option<bool>,
        pagination: Pagination,
    ) -> Result<Vec<T>, surrealdb::Error>;

    /// Get task requests created by user
    async fn get_by_creator<T: for<'de> Deserialize<'de> + SurrealValue + ViewFieldSelector + Send>(
        &self,
        user: RecordId,
        pagination: Pagination,
    ) -> Result<Vec<T>, surrealdb::Error>;

    /// Update task request status
    async fn update_status(
        &self,
        task_id: &str,
        status: TaskRequestStatus,
    ) -> Result<(), surrealdb::Error>;

    /// Get task ready for payment by ID
    async fn get_ready_for_payment_by_id(
        &self,
        id: &str,
    ) -> Result<TaskForReward, surrealdb::Error>;

    /// Get all tasks ready for payment
    async fn get_ready_for_payment(&self) -> Result<Vec<TaskForReward>, surrealdb::Error>;

    async fn get_by_id<T: for<'de> Deserialize<'de> + SurrealValue + ViewFieldSelector + Send>(
        &self,
        id: &str,
    ) -> AppResult<T>;
}
