use crate::database::client::Db;
use crate::database::repository_traits::{EntityWithId, RepositoryCore, RepositoryEntityId};
use crate::database::surrdb_utils;
use crate::middleware::utils::db_utils::{IdentIdName, Pagination};
use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;
use std::{marker::PhantomData, string::String};
use surrealdb::sql::{Id, Thing};

#[derive(Debug)]
pub struct Repository<E> {
    pub client: Arc<Db>,
    pub table_name: String,
    _phantom: PhantomData<E>,
}

#[async_trait]
impl<E: Serialize + for<'de> serde::Deserialize<'de> + Send + Sync + 'static> RepositoryCore
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

    async fn query(&self, query: &str) -> Result<Self::QueryResultList, Self::Error> {
        let mut result = self.client.query(query).await?;
        let values: Self::QueryResultList = result.take(0)?;
        Ok(values)
    }

    async fn create_entity(
        &self,
        entity: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error> {
        let res: Option<Self::QueryResultItem> =
            self.client.create(&self.table_name).content(entity).await?;
        Ok(res.unwrap())
    }

    async fn select_by_id(&self, record_id: &str) -> Result<Self::QueryResultItem, Self::Error> {
        let res: Option<Self::QueryResultItem> =
            self.client.select((&self.table_name, record_id)).await?;
        Ok(res.unwrap())
    }

    async fn get_entity(
        &self,
        ident: &IdentIdName,
    ) -> Result<Option<Self::QueryResultItem>, Self::Error> {
        let query_string = surrdb_utils::get_entity_query_str(ident, Some("*"), None, self.table_name.as_ref())?;
        println!("QRY={:#?}", query_string);
        surrdb_utils::get_query(self.client.as_ref(), query_string).await
    }

    async fn delete_by_id(&self, record_id: &str) -> Result<bool, surrealdb::Error> {
        let _res: Option<Self::QueryResultItem> =
            self.client.delete((&self.table_name, record_id)).await?;
        Ok(true)
    }

    async fn count_records(&self) -> Result<u64, surrealdb::Error> {
        let query = format!(
            "(SELECT count() as count FROM ONLY {} GROUP ALL).count;",
            self.table_name
        );
        let mut res = self.client.query(&query).await?;
        let res: Option<u64> = res.take(0)?;
        res.ok_or(
            surrealdb::error::Db::TbNotFound {
                name: format!("table {}", self.table_name),
            }
            .into(),
        )
    }

    fn get_thing(&self, id: &str) -> Thing {
        Thing::from((self.table_name.as_ref(), id))
    }

    async fn get_entity_list(
        &self,
        ident: &IdentIdName,
        pagination: Option<Pagination>,
    ) -> Result<Self::QueryResultList, surrealdb::Error> {
        let query_string =
            surrdb_utils::get_entity_query_str(ident, Some("*"), pagination, self.table_name.as_ref())?;

        surrdb_utils::get_list_qry(self.client.as_ref(), query_string).await
    }

    async fn exists_entity(&self, ident: &IdentIdName) -> Result<Thing, Self::Error> {
        surrdb_utils::exists_entity(self.client.as_ref(), self.table_name.as_ref(), ident).await
    }

    async fn exists_record(&self, record_id: &Thing) -> Result<(), Self::Error> {
        surrdb_utils::record_exists(self.client.as_ref(), record_id).await
    }

    async fn record_exist_all(&self, record_ids: Vec<String>) -> Result<Vec<Thing>, Self::Error> {
        surrdb_utils::record_exist_all(self.client.as_ref(), record_ids).await
    }
}

#[async_trait]
impl<E: EntityWithId + Serialize + for<'de> serde::Deserialize<'de> + Send + Sync + 'static>
    RepositoryEntityId for Repository<E>
{
    async fn update(
        &self,
        entity: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error> {
        let id_str = entity
            .id_str()
            .ok_or(Self::Error::from(surrealdb::error::Db::IdInvalid {
                value: "no id set".to_string(),
            }))?;
        let res: Option<Self::QueryResultItem> = self
            .client
            .update((&self.table_name, id_str))
            .content(entity)
            .await?;
        Ok(res.unwrap())
    }

    async fn create_update(
        &self,
        record: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error> {
        let id = if let Some(id) = record.id_str() {
            Id::from(id)
        } else {
            Id::rand()
        };

        let res: Option<Self::QueryResultItem> = self
            .client
            .upsert((self.table_name.clone(), id.to_raw()))
            .content(record)
            .await?;
        Ok(res.unwrap())
    }
}
