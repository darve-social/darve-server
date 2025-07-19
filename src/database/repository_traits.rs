use crate::middleware::utils::db_utils::{IdentIdName, Pagination};
use async_trait::async_trait;
use std::string::String;
use surrealdb::sql::Thing;

#[async_trait]
pub trait RepositoryCore {
    type Connection;
    type Error: std::error::Error + Send;
    type QueryResultItem;
    type QueryResultList; // = Vec<Self::QueryResultItem>; (not stable yet)

    fn new(conn: Self::Connection, table_name: String) -> Self
    where
        Self: Sized;

    async fn query(&self, query: &str) -> Result<Self::QueryResultList, Self::Error>;
    async fn create_entity(
        &self,
        entity: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error>;

    async fn select_by_id(&self, record_id: &str) -> Result<Self::QueryResultItem, Self::Error>;
    async fn get_entity(
        &self,
        ident: &IdentIdName,
    ) -> Result<Option<Self::QueryResultItem>, Self::Error>;

    async fn delete(&self, record_id: &str) -> Result<bool, Self::Error>;
    async fn count(&self) -> Result<u64, Self::Error>;
    fn get_thing(&self, id: &str) -> Thing;
    async fn get_entity_list(
        &self,
        ident: &IdentIdName,
        pagination: Option<Pagination>,
    ) -> Result<Self::QueryResultList, Self::Error>;
    async fn exists_entity(&self, ident: &IdentIdName) -> Result<Option<Thing>, Self::Error>;
    async fn record_exists(&self, record_id: &Thing) -> Result<(), Self::Error>;
    async fn record_exist_all(&self, record_ids: Vec<String>) -> Result<Vec<Thing>, Self::Error>;
}

#[async_trait]
pub trait RepositoryEntityId: RepositoryCore {
    async fn update(
        &self,
        entity: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error>;
    async fn create_update(
        &self,
        record: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error>;
}

pub trait EntityWithId {
    fn id_str(&self) -> Option<&str>;
}
