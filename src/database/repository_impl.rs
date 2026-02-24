use crate::database::client::Db;
use crate::database::repository_traits::RepositoryEntityView;
use crate::database::repository_traits::{
    EntityWithId, RepositoryConn, RepositoryCore, RepositoryEntityId,
};
use crate::database::surrdb_utils;
use crate::middleware::utils::db_utils::{IdentIdName, Pagination, ViewFieldSelector};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{marker::PhantomData, string::String};
use surrealdb::types::{RecordId, RecordIdKey, SurrealValue};
use surrealdb::Error;

#[derive(Debug)]
pub struct Repository<E> {
    pub client: Arc<Db>,
    pub table_name: String,
    _phantom: PhantomData<E>,
}

#[async_trait]
impl<E: Serialize + for<'de> serde::Deserialize<'de> + SurrealValue + Send + Sync + 'static> RepositoryConn
    for Repository<E>
{
    type Connection = Arc<Db>;
    type Error = surrealdb::Error;
    type QueryResultItem = E;
    type QueryResultList = Vec<Self::QueryResultItem>;

    fn new(client: Self::Connection, table_name: String) -> Self {
        Repository {
            client,
            table_name,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<E: Serialize + for<'de> serde::Deserialize<'de> + SurrealValue + Send + Sync + 'static> RepositoryCore
    for Repository<E>
{
    async fn item_create(
        &self,
        entity: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error> {
        let res: Option<Self::QueryResultItem> =
            self.client.create(&self.table_name).content(entity).await?;
        Ok(res.unwrap())
    }

    async fn item_by_id(&self, id: &str) -> Result<Self::QueryResultItem, Self::Error> {
        let res: Option<Self::QueryResultItem> = self.client.select((&self.table_name, id)).await?;
        Ok(res.unwrap())
    }

    async fn item_by_ident(
        &self,
        ident: &IdentIdName,
    ) -> Result<Option<Self::QueryResultItem>, Self::Error> {
        surrdb_utils::get_entity(self.client.as_ref(), self.table_name.as_ref(), ident).await
    }

    async fn list_by_ids(&self, ids: Vec<&str>) -> Result<Vec<Self::QueryResultItem>, Self::Error> {
        let things = ids.into_iter().map(|id| self.get_thing(id)).collect();
        surrdb_utils::get_entities_by_id(self.client.as_ref(), things).await
    }

    async fn item_delete(&self, record_id: &str) -> Result<bool, surrealdb::Error> {
        let _res: Option<Self::QueryResultItem> =
            self.client.delete((&self.table_name, record_id)).await?;
        Ok(true)
    }

    async fn count_records(&self) -> Result<u64, surrealdb::Error> {
        surrdb_utils::count_records(self.client.as_ref(), self.table_name.as_ref()).await
    }

    fn get_thing(&self, id: &str) -> RecordId {
        RecordId::new(self.table_name.as_str(), id)
    }

    async fn list_by_ident(
        &self,
        ident: &IdentIdName,
        pagination: Option<Pagination>,
    ) -> Result<Self::QueryResultList, surrealdb::Error> {
        surrdb_utils::get_entity_list(
            self.client.as_ref(),
            self.table_name.as_ref(),
            ident,
            pagination,
        )
        .await
    }

    async fn item_ident_exists(&self, ident: &IdentIdName) -> Result<RecordId, Self::Error> {
        surrdb_utils::exists_entity(self.client.as_ref(), self.table_name.as_ref(), ident).await
    }

    async fn item_id_exists(&self, id: &str) -> Result<(), Self::Error> {
        surrdb_utils::exists_by_thing(self.client.as_ref(), &self.get_thing(id)).await
    }

    async fn items_exist_all(&self, record_ids: Vec<&str>) -> Result<Vec<RecordId>, Self::Error> {
        let thing_strs = record_ids
            .into_iter()
            .map(|id| format!("{}:{}", self.table_name, id))
            .collect();
        surrdb_utils::record_exist_all(self.client.as_ref(), thing_strs).await
    }

    async fn list_view_by_ident<T: for<'a> Deserialize<'a> + SurrealValue + ViewFieldSelector>(
        &self,
        ident: &IdentIdName,
        pagination: Option<Pagination>,
    ) -> Result<Vec<T>, Error> {
        surrdb_utils::get_entity_list_view::<T>(
            self.client.as_ref(),
            self.table_name.as_ref(),
            ident,
            pagination,
        )
        .await
    }
    async fn item_view_by_ident<T: for<'a> Deserialize<'a> + SurrealValue + ViewFieldSelector>(
        &self,
        ident: &IdentIdName,
    ) -> Result<Option<T>, Error> {
        surrdb_utils::get_entity_view::<T>(self.client.as_ref(), self.table_name.as_ref(), ident)
            .await
    }
}

#[async_trait]
impl<E: EntityWithId + Serialize + for<'de> serde::Deserialize<'de> + SurrealValue + Send + Sync + 'static>
    RepositoryEntityId for Repository<E>
{
    async fn update_entity(
        &self,
        entity: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error> {
        let id_str = entity
            .id_str()
            .ok_or(Self::Error::validation("no id set".to_string(), None))?;
        let res: Option<Self::QueryResultItem> = self
            .client
            .update((&self.table_name, id_str))
            .content(entity)
            .await?;
        Ok(res.unwrap())
    }

    async fn create_update_entity(
        &self,
        record: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error> {
        let id = if let Some(id) = record.id_str() {
            RecordIdKey::from(id.to_string())
        } else {
            RecordIdKey::rand()
        };

        let id_str = match &id {
            RecordIdKey::String(s) => s.clone(),
            RecordIdKey::Number(n) => n.to_string(),
            RecordIdKey::Uuid(u) => u.to_string(),
            _ => String::new(),
        };

        let res: Option<Self::QueryResultItem> = self
            .client
            .upsert((self.table_name.clone(), id_str))
            .content(record)
            .await?;
        Ok(res.unwrap())
    }
}

#[derive(Debug)]
pub struct RepositoryView<E> {
    pub client: Arc<Db>,
    pub table_name: String,
    _phantom: PhantomData<E>,
}
#[async_trait]
impl<E: for<'de> serde::Deserialize<'de> + SurrealValue + Send + Sync + 'static + ViewFieldSelector> RepositoryConn
    for RepositoryView<E>
{
    type Connection = Arc<Db>;
    type Error = surrealdb::Error;
    type QueryResultItem = E;
    type QueryResultList = Vec<Self::QueryResultItem>;

    fn new(client: Self::Connection, table_name: String) -> Self {
        RepositoryView {
            client,
            table_name,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<E: for<'de> serde::Deserialize<'de> + SurrealValue + Send + Sync + 'static + ViewFieldSelector>
    RepositoryEntityView for RepositoryView<E>
{
    async fn list_view(
        &self,
        ident: &IdentIdName,
        pagination: Option<Pagination>,
    ) -> Result<Self::QueryResultList, Error> {
        surrdb_utils::get_entity_list_view::<Self::QueryResultItem>(
            self.client.as_ref(),
            self.table_name.as_ref(),
            ident,
            pagination,
        )
        .await
    }

    async fn get_entity_view(
        &self,
        ident: &IdentIdName,
    ) -> Result<Option<Self::QueryResultItem>, Error> {
        surrdb_utils::get_entity_view::<Self::QueryResultItem>(
            self.client.as_ref(),
            self.table_name.as_ref(),
            ident,
        )
        .await
    }
}
