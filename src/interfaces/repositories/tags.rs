use async_trait::async_trait;
use serde::Deserialize;
use surrealdb::types::{RecordId, SurrealValue};

use crate::{
    entities::tag::Tag,
    middleware::{error::AppResult, utils::db_utils::{Pagination, ViewRelateField}},
};

#[async_trait]
pub trait TagsRepositoryInterface {
    async fn create_with_relate(&self, tags: Vec<String>, entity: RecordId) -> AppResult<()>;
    async fn get_by_tag<T: for<'de> Deserialize<'de> + SurrealValue + ViewRelateField>(
        &self,
        tag: &str,
        pad: Pagination,
    ) -> AppResult<Vec<T>>;
    async fn get(&self, start_with: Option<String>, pad: Pagination) -> AppResult<Vec<Tag>>;
}
