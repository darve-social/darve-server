use crate::database::client::Db;
use crate::entities::task::task_request_participation_entity::TaskRequestParticipation;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{marker::PhantomData, string::String};
use surrealdb::sql::{Id, Thing};
use crate::middleware::error::{AppError, AppResult, CtxResult};
use crate::middleware::utils::db_utils;
use crate::middleware::utils::db_utils::{get_entity_query_str, get_list_qry, record_exists, IdentIdName, Pagination, RecordWithId, ViewFieldSelector};

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
    async fn create(
        &self,
        entity: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error>;
    async fn update(
        &self,
        entity: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error>;
    async fn select_by_id(
        &self,
        record_id: &str,
    ) -> Result<Self::QueryResultItem, Self::Error>;

    async fn delete(&self, record_id: &str) -> Result<bool, Self::Error>;

    async fn create_update(
        &self,
        record: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error>;
    async fn count(&self) -> Result<u64, surrealdb::Error>;
    async fn get_thing(&self, id: &str) -> Thing;

    async fn get_entity_view< T: for<'a> Deserialize<'a> + ViewFieldSelector>(&self, ident: &IdentIdName) -> CtxResult<Option<T>>;
    async fn get_entity_list<T: for<'a> Deserialize<'a>>(
        &self,
        ident: &IdentIdName,
        pagination: Option<Pagination>,
    ) -> CtxResult<Vec<T>>;
    async fn get_entity_list_view<T: for<'a> Deserialize<'a> + ViewFieldSelector>(
        &self,
        ident: &IdentIdName,
        pagination: Option<Pagination>,
    ) -> CtxResult<Vec<T>>;
    async fn exists_entity(
        &self,
        ident: &IdentIdName,
    ) -> CtxResult<Option<Thing>>;
    async fn record_exists(&self, record_id: &Thing) -> AppResult<()>;
    async fn record_exist_all(&self, record_ids: Vec<String>) -> AppResult<Vec<Thing>>;
}

pub trait EntityWithId {
    fn id_str(&self) -> Option<&str>;
}

#[derive(Debug)]
pub struct Repository<E> {
    pub client: Arc<Db>,
    pub table_name: String,
    _phantom: PhantomData<E>,
}

#[async_trait]
impl<
        E: EntityWithId + Serialize + for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
    > RepositoryCore for Repository<E>
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

    async fn create(
        &self,
        entity: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error> {
        let res: Option<Self::QueryResultItem> =
            self.client.create(&self.table_name).content(entity).await?;
        Ok(res.unwrap())
    }
    
    async fn update(
        &self,
        entity: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error> {
        let id_str = entity.id_str().ok_or(Self::Error::from(surrealdb::error::Db::IdInvalid {value:"no id set".to_string()}))?;
        let res: Option<Self::QueryResultItem> =
            self.client.update((&self.table_name, id_str)).content(entity).await?;  
        Ok(res.unwrap())
    }

    async fn select_by_id(
        &self,
        record_id: &str,
    ) -> Result<Self::QueryResultItem, Self::Error> {
        let res: Option<Self::QueryResultItem> =
            self.client.select((&self.table_name, record_id)).await?;
        Ok(res.unwrap())
    }

    async fn delete(&self, record_id: &str) -> Result<bool, surrealdb::Error> {
        let _res: Option<TaskRequestParticipation> = self
            .client
            .delete((&self.table_name, record_id))
            .await?;
        Ok(true)
    }

    async fn create_update(
        &self,
        record: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, surrealdb::Error> {
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

    async fn count(&self) -> Result<u64, surrealdb::Error> {
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
    
    async fn get_thing(&self, id: &str) -> Thing {
        Thing::from((self.table_name.as_ref(), id))
    }


    async fn get_entity_view<T: for<'a> Deserialize<'a> + ViewFieldSelector>(
        &self,
        ident: &IdentIdName,
    ) -> CtxResult<Option<T>> {
        let query_string = db_utils::get_entity_query_str(
            ident,
            Some(T::get_select_query_fields(ident).as_str()),
            None,
            self.table_name.clone(),
        )?;
        db_utils::get_query(self.client.as_ref(), query_string).await
    }


    async fn get_entity_list<T: for<'a> Deserialize<'a>>(
        &self,
        ident: &IdentIdName,
        pagination: Option<Pagination>,
    ) -> CtxResult<Vec<T>> {
        let query_string = get_entity_query_str(ident, Some("*"), pagination, self.table_name.clone())?;

        get_list_qry(self.client.as_ref(), query_string).await
    }

    async fn get_entity_list_view<T: for<'a> Deserialize<'a> + ViewFieldSelector>(
        &self,
        ident: &IdentIdName,
        pagination: Option<Pagination>,
    ) -> CtxResult<Vec<T>> {
        let query_string = get_entity_query_str(
            ident,
            Some(T::get_select_query_fields(ident).as_str()),
            pagination,
            self.table_name.clone(),
        )?;
        get_list_qry(self.client.as_ref(), query_string).await
    }

    async fn exists_entity(
        &self,
        ident: &IdentIdName,
    ) -> CtxResult<Option<Thing>> {
        match ident {
            IdentIdName::Id(id) => {
                record_exists(self.client.as_ref(), id).await?;
                Ok(Some(id.clone()))
            }
            _ => {
                let query_string = get_entity_query_str(ident, None, None, self.table_name.clone())?;
                let qry = db_utils::create_db_qry(self.client.as_ref(), query_string);

                let mut res = qry.await?;
                let res = res.take::<Option<RecordWithId>>(0)?;
                match res {
                    None => Ok(None),
                    Some(rec) => Ok(Some(rec.id)),
                }
            }
        }
    }

    async fn record_exists(&self, record_id: &Thing) -> AppResult<()> {
        let qry = "RETURN record::exists(<record>$rec_id);";
        let mut res = self.client.as_ref().query(qry).bind(("rec_id", record_id.to_raw())).await?;
        let res: Option<bool> = res.take(0)?;
        match res.unwrap_or(false) {
            true => Ok(()),
            false => Err(AppError::EntityFailIdNotFound {
                ident: record_id.to_raw(),
            }),
        }
    }

    async fn record_exist_all(&self, record_ids: Vec<String>) -> AppResult<Vec<Thing>> {
        if record_ids.is_empty() {
            return Ok(vec![]);
        }

        let things = record_ids
            .iter()
            .map(|rec_id| {
                Thing::try_from(rec_id.as_str()).map_err(|_| AppError::Generic {
                    description: format!("Invalid record id = {}", rec_id),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let conditions = (0..things.len())
            .map(|i| format!("record::exists(<record>$rec_id_{i})"))
            .collect::<Vec<_>>()
            .join(" AND ");

        let query = {
            let query_str = format!("RETURN {conditions};");
            let mut query = self.client.as_ref().query(query_str);

            for (i, val) in things.iter().enumerate() {
                query = query.bind((format!("rec_id_{i}"), val.clone()));
            }

            query
        };

        let mut res = query.await?;
        let exists: Option<bool> = res.take(0)?;

        if !exists.unwrap_or(false) {
            return Err(AppError::EntityFailIdNotFound {
                ident: "Not all ids exist".to_string(),
            });
        }

        Ok(things)
    }
}
