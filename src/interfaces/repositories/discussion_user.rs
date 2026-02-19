use crate::{
    database::query_builder::SurrealQueryBuilder,
    entities::discussion_user::DiscussionUser,
    middleware::{
        error::AppResult,
        utils::db_utils::{Pagination, ViewFieldSelector},
    },
};
use async_trait::async_trait;
use serde::Deserialize;
use surrealdb::types::{RecordId, SurrealValue};

#[async_trait]
pub trait DiscussionUserRepositoryInterface {
    async fn create(&self, disc_id: &str, user_ids: Vec<RecordId>) -> AppResult<()>;

    async fn set_new_latest_post(
        &self,
        disc_id: &str,
        user_ids: Vec<&String>,
        latest_post: &str,
        increase_unread_for_user_ids: Vec<&String>,
    ) -> AppResult<Vec<DiscussionUser>>;

    /// * `disc_id` - The ID (id of thing) of the discussion to update
    /// * `user_ids` - A vector of user IDs (id of thing) whose discussion records should be updated
    async fn update_latest_post(
        &self,
        disc_id: &str,
        user_ids: Vec<String>,
    ) -> AppResult<Vec<DiscussionUser>>;

    async fn update_alias(
        &self,
        disc_id: &str,
        user_id: &str,
        alias: Option<String>,
    ) -> AppResult<()>;

    async fn decrease_unread_count(
        &self,
        disc_id: &str,
        user_ids: Vec<String>,
    ) -> AppResult<Vec<DiscussionUser>>;

    fn build_decrease_query(
        &self,
        query: SurrealQueryBuilder,
        disc_id: &str,
        user_ids: Vec<String>,
    ) -> SurrealQueryBuilder;

    async fn remove(&self, disc_id: &str, user_ids: Vec<RecordId>) -> AppResult<Vec<RecordId>>;

    async fn get_count_of_unread(&self, user_id: &str) -> AppResult<u32>;

    async fn get_by_user<T: for<'b> Deserialize<'b> + SurrealValue + ViewFieldSelector>(
        &self,
        user_id: &str,
        pad: Pagination,
        require_latest_post: bool,
        search_text: Option<String>,
    ) -> AppResult<Vec<T>>;
}
