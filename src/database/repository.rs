use crate::database::client::Db;
use crate::entities::task::task_request_participation_entity::TaskRequestParticipation;
use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;
use std::{marker::PhantomData, string::String};
use surrealdb::sql::{Id, Thing};

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

    async fn delete(&self, participation_id: Thing) -> Result<bool, Self::Error>;

    async fn create_update(
        &self,
        record: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, Self::Error>;
    async fn count(&self) -> Result<u64, surrealdb::Error>;
}

pub trait OptionalIdentifier {
    fn ident_ref(&self) -> Option<&Thing>;
}

#[derive(Debug)]
pub struct Repository<E> {
    pub client: Arc<Db>,
    pub table_name: String,
    _phantom: PhantomData<E>,
}

#[async_trait]
impl<
        E: OptionalIdentifier + Serialize + for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
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

    async fn delete(&self, participation_id: Thing) -> Result<bool, surrealdb::Error> {
        let _res: Option<TaskRequestParticipation> = self
            .client
            .delete((&self.table_name, participation_id.id.to_raw()))
            .await?;
        Ok(true)
    }

    async fn create_update(
        &self,
        record: Self::QueryResultItem,
    ) -> Result<Self::QueryResultItem, surrealdb::Error> {
        let id: String = if let Some(thing) = record.ident_ref() {
            thing.id.clone().to_raw()
        } else {
            Id::rand().to_raw()
        };

        let res: Option<Self::QueryResultItem> = self
            .client
            .upsert((self.table_name.as_str(), id))
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
}
