use crate::database::client::Db;
use crate::interfaces::repositories::tags::TagsRepositoryInterface;
use crate::middleware::error::{AppError, AppResult};
use crate::middleware::utils::db_utils::{Pagination, QryOrder};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use surrealdb::sql::Thing;

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
        DEFINE TABLE IF NOT EXISTS tag TYPE RELATION IN tags ENFORCED SCHEMAFULL PERMISSIONS NONE;
        DEFINE TABLE IF NOT EXISTS tags SCHEMAFULL;
    "
        );
        let mutation = self.client.query(sql).await?;

        mutation.check().expect("should mutate TagsRepository");

        Ok(())
    }
}

#[async_trait]
impl TagsRepositoryInterface for TagsRepository {
    async fn create_with_relate(&self, tags: Vec<String>, thing: Thing) -> AppResult<()> {
        let _ = self
            .client
            .query("LET $ids = UPSERT $tags.map(|$v| type::thing('tags', $v));")
            .query("RELATE $ids->tag->$entity;")
            .query("RETURN $ids;")
            .bind(("tags", tags))
            .bind(("entity", thing))
            .await?;
        Ok(())
    }

    async fn get_by_tag<T: for<'de> Deserialize<'de>>(
        &self,
        tag: &str,
        pag: Pagination,
    ) -> AppResult<Vec<T>> {
        let order_dir = pag.order_dir.unwrap_or(QryOrder::DESC).to_string();
        let order_by = pag.order_by.unwrap_or("id".to_string()).to_string();
        let query = format!(
            "SELECT *, out.* AS entity FROM $tag->tag ORDER BY out.{} {} LIMIT $limit START $start;",
            order_by,
            order_dir
        );
        let mut res = self
            .client
            .query(query)
            .bind(("tag", Thing::from(("tags", tag))))
            .bind(("limit", pag.count))
            .bind(("start", pag.start))
            .await?;

        let data = res.take::<Vec<T>>((0, "entity"))?;

        Ok(data)
    }

    async fn get(&self, start_with: Option<String>, pag: Pagination) -> AppResult<Vec<String>> {
        let dir = pag.order_dir.unwrap_or(QryOrder::DESC).to_string();
        let where_condition = if start_with.is_some() {
            "WHERE string::starts_with(record::id(id), $value)"
        } else {
            ""
        };
        let query = format!(
            "SELECT record::id(id) as tag, math::sum(->tag.out.likes_nr) AS count FROM tags
            {where_condition}
            ORDER BY count {dir}, tag ASC LIMIT $limit START $start;",
        );
        let mut res = self
            .client
            .query(query)
            .bind(("value", start_with))
            .bind(("limit", pag.count))
            .bind(("start", pag.start))
            .await?;

        println!(">>>>.{:?}", res);
        let data = res.take::<Vec<String>>((0, "tag"))?;
        Ok(data)
    }
}
