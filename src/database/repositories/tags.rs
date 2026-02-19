use crate::database::client::Db;
use crate::database::table_names::{TAG_REL_TABLE_NAME, TAG_TABLE_NAME};
use crate::entities::tag::Tag;
use crate::interfaces::repositories::tags::TagsRepositoryInterface;
use crate::middleware::error::{AppError, AppResult};
use crate::middleware::utils::db_utils::{Pagination, QryOrder};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug)]
pub struct TagsRepository {
    client: Arc<Db>,
}

impl TagsRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self { client }
    }
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!(
            "
            DEFINE TABLE IF NOT EXISTS {TAG_REL_TABLE_NAME} TYPE RELATION IN {TAG_TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
            DEFINE TABLE IF NOT EXISTS {TAG_TABLE_NAME} SCHEMAFULL;
            "
        );
        let mutation = self.client.query(sql).await?;

        mutation.check().expect("should mutate TagsRepository");

        Ok(())
    }
}

#[async_trait]
impl TagsRepositoryInterface for TagsRepository {
    async fn create_with_relate(&self, tags: Vec<String>, entity: RecordId) -> AppResult<()> {
        let _ = self
            .client
            .query(format!(
                "BEGIN TRANSACTION; \
                LET $ids = UPSERT $tags.map(|$v| type::thing('{TAG_TABLE_NAME}', $v)); \
                RELATE $ids->{TAG_REL_TABLE_NAME}->$entity; \
                RETURN $ids; \
                COMMIT TRANSACTION;"
            ))
            .bind(("tags", tags))
            .bind(("entity", entity))
            .await?;
        Ok(())
    }

    async fn get_by_tag<T: for<'de> Deserialize<'de> + SurrealValue>(
        &self,
        tag: &str,
        pag: Pagination,
    ) -> AppResult<Vec<T>> {
        let order_dir = pag.order_dir.unwrap_or(QryOrder::DESC).to_string();
        let order_by = pag.order_by.unwrap_or("id".to_string()).to_string();
        let query = format!(
            "SELECT *, out.* AS entity FROM $tag->{TAG_REL_TABLE_NAME} ORDER BY out.{} {} LIMIT $limit START $start;",
            order_by,
            order_dir
        );
        let mut res = self
            .client
            .query(query)
            .bind(("tag", RecordId::new(TAG_TABLE_NAME, tag)))
            .bind(("limit", pag.count))
            .bind(("start", pag.start))
            .await?;

        let data = res.take::<Vec<T>>((0, "entity"))?;

        Ok(data)
    }

    async fn get(&self, start_with: Option<String>, pag: Pagination) -> AppResult<Vec<Tag>> {
        let dir = pag.order_dir.unwrap_or(QryOrder::DESC).to_string();
        let where_condition = if start_with.is_some() {
            "WHERE string::starts_with(string::lowercase(record::id(id)), $value)"
        } else {
            ""
        };
        let query = format!(
            "SELECT *,  record::id(id) as tag, math::sum(->{TAG_REL_TABLE_NAME}.out.likes_nr) AS count FROM {TAG_TABLE_NAME}
            {where_condition}
            ORDER BY count {dir}, tag ASC LIMIT $limit START $start;",
        );
        let mut res = self
            .client
            .query(query)
            .bind(("value", start_with.map(|v| v.to_lowercase())))
            .bind(("limit", pag.count))
            .bind(("start", pag.start))
            .await?;

        let data = res.take::<Vec<Tag>>(0)?;
        Ok(data)
    }
}
