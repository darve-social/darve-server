use async_trait::async_trait;
use serde::Deserialize;
use surrealdb::sql::Thing;

use crate::{
    entities::tag::Tag,
    middleware::{error::AppResult, utils::db_utils::Pagination},
};

#[async_trait]
pub trait TagsRepositoryInterface {
    async fn create_with_relate(&self, tags: Vec<String>, entity: Thing) -> AppResult<()>;
    async fn get_by_tag<T: for<'de> Deserialize<'de>>(
        &self,
        tag: &str,
        pad: Pagination,
    ) -> AppResult<Vec<T>>;
    async fn get(&self, start_with: Option<String>, pad: Pagination) -> AppResult<Vec<Tag>>;
}
