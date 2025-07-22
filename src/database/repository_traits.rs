use crate::middleware::utils::db_utils::{IdentIdName, Pagination, ViewFieldSelector};
use async_trait::async_trait;
use serde::Deserialize;
use std::string::String;
use surrealdb::sql::Thing;

pub trait RepositoryConn {
    type Connection;
    type Error: std::error::Error + Send;
    type QueryResultItem;
    type QueryResultList;
    
    fn new(conn: Self::Connection, table_name: String) -> Self
    where
        Self: Sized;
}

#[async_trait]
pub trait RepositoryCore: RepositoryConn {
    
    async fn item_create(
        &self,
        entity: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error>;

    async fn item_by_id(&self, record_id: &str) -> Result<Self::QueryResultItem, Self::Error>;
    async fn item_by_ident(
        &self,
        ident: &IdentIdName,
    ) -> Result<Option<Self::QueryResultItem>, Self::Error>;
    async fn list_by_ids(
        &self,
        ids: Vec<&str>,
    ) -> Result<Vec<Self::QueryResultItem>, Self::Error>;

    async fn item_delete(&self, record_id: &str) -> Result<bool, Self::Error>;
    async fn count_records(&self) -> Result<u64, Self::Error>;
    fn get_thing(&self, id: &str) -> Thing;
    async fn list_by_ident(
        &self,
        ident: &IdentIdName,
        pagination: Option<Pagination>,
    ) -> Result<Self::QueryResultList, Self::Error>;
    async fn item_ident_exists(&self, ident: &IdentIdName) -> Result<Thing, Self::Error>;
    async fn item_id_exists(&self, record_id: &str) -> Result<(), Self::Error>;
    async fn items_exist_all(&self, record_ids: Vec<&str>) -> Result<Vec<Thing>, Self::Error>;
    
    async fn list_view_by_ident<T: for<'a> Deserialize<'a> + ViewFieldSelector>(&self,
                                                                                ident: &IdentIdName,
                                                                                pagination: Option<Pagination>,
    ) -> Result<Vec<T>, surrealdb::Error>;
    async fn item_view_by_ident<T: for<'a> Deserialize<'a> + ViewFieldSelector>(&self,
                                                                                ident: &IdentIdName,
    ) -> Result<Option<T>, surrealdb::Error>;
}

#[async_trait]
pub trait RepositoryEntityId: RepositoryCore {
    async fn update_entity(
        &self,
        entity: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error>;
    async fn create_update_entity(
        &self,
        record: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error>;
}

pub trait EntityWithId {
    fn id_str(&self) -> Option<&str>;
}

#[async_trait]
pub trait RepositoryEntityView: RepositoryConn {
    async fn list_view(&self,
                                                                                  ident: &IdentIdName,
                                                                                  pagination: Option<Pagination>,
    ) -> Result<Self::QueryResultList, surrealdb::Error>;
    async fn get_entity_view(&self,
                             ident: &IdentIdName,
    ) -> Result<Option<Self::QueryResultItem>, surrealdb::Error>;
}