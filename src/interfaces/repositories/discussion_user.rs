use crate::middleware::{
    error::AppResult,
    utils::db_utils::{Pagination, ViewFieldSelector},
};
use async_trait::async_trait;
use serde::Deserialize;
use surrealdb::sql::Thing;

#[async_trait]
pub trait DiscussionUserRepositoryInterface {
    async fn create(&self, disc_id: &str, user_ids: Vec<Thing>) -> AppResult<()>;

    async fn set_new_latest_post(
        &self,
        disc_id: &str,
        user_ids: Vec<&String>,
        latest_post: &str,
        increase_unread_for_user_ids: Vec<&String>,
    ) -> AppResult<()>;

    async fn update_latest_post(&self, disc_id: &str, user_ids: Vec<String>) -> AppResult<()>;

    async fn decrease_unread_count(&self, disc_id: &str, user_ids: Vec<String>) -> AppResult<()>;

    async fn remove(&self, disc_id: &str, user_ids: Vec<Thing>) -> AppResult<()>;

    async fn get_by_user<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        user_id: &str,
        pad: Pagination,
        require_latest_post: bool,
    ) -> AppResult<Vec<T>>;
}
