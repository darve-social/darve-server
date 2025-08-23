use crate::database::client::Db;
use crate::database::table_names::REPLY_TABLE_NAME;
use crate::entities::community::post_entity::TABLE_NAME as POST_TABLE_NAME;
use crate::entities::reply::Reply;
use crate::entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME;
use crate::middleware::error::{AppError, AppResult};
use crate::middleware::utils::db_utils::{Pagination, QryOrder, ViewFieldSelector};
use crate::models::view::reply::ReplyView;
use std::sync::Arc;
use surrealdb::sql::Thing;

#[derive(Debug)]
pub struct RepliesRepository {
    client: Arc<Db>,
}

impl RepliesRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self { client }
    }
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {REPLY_TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS belongs_to ON TABLE {REPLY_TABLE_NAME} TYPE record<{POST_TABLE_NAME}>;
    DEFINE INDEX IF NOT EXISTS belongs_to_idx ON TABLE {REPLY_TABLE_NAME} COLUMNS belongs_to;
    DEFINE FIELD IF NOT EXISTS created_by ON TABLE {REPLY_TABLE_NAME} TYPE record<{USER_TABLE_NAME}>;
    DEFINE FIELD IF NOT EXISTS content ON TABLE {REPLY_TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD IF NOT EXISTS likes_nr ON TABLE {REPLY_TABLE_NAME} TYPE number DEFAULT 0;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE {REPLY_TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS updated_at ON TABLE {REPLY_TABLE_NAME}  TYPE datetime DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.client.query(sql).await?;

        mutation.check().expect("should mutate RepliesRepository");

        Ok(())
    }

    pub async fn create(&self, post_id: &str, user_id: &str, content: &str) -> AppResult<Reply> {
        let mut res = self
            .client
           .query(format!("INSERT INTO {REPLY_TABLE_NAME} {{id:rand::ulid(), belongs_to: $post, created_by: $user, content: $content }}"))
            .bind(("user", Thing::from((USER_TABLE_NAME, user_id))))
            .bind(("post", Thing::from((POST_TABLE_NAME, post_id))))
            .bind(("content", content.to_string()))
            .await?;

        let record = res.take::<Option<Reply>>(0)?;

        Ok(record.unwrap())
    }

    pub async fn get(
        &self,
        user_id: &str,
        post_id: &str,
        pagination: Pagination,
    ) -> AppResult<Vec<ReplyView>> {
        let order_dir = pagination.order_dir.unwrap_or(QryOrder::DESC).to_string();
        let fields = ReplyView::get_select_query_fields();
        let data = self
            .client
            .query(
                format!(
                    "SELECT {fields} FROM {REPLY_TABLE_NAME}
                            WHERE belongs_to=$post
                            ORDER BY id {order_dir} LIMIT $limit START $start;"
                )
                .as_str(),
            )
            .bind(("post", Thing::from((POST_TABLE_NAME, post_id))))
            .bind(("user", Thing::from((USER_TABLE_NAME, user_id))))
            .bind(("limit", pagination.count))
            .bind(("start", pagination.start))
            .await?
            .take::<Vec<ReplyView>>(0)?;

        Ok(data)
    }

    pub async fn get_by_id(&self, reply_id: &str) -> AppResult<Reply> {
        let data: Option<Reply> = self.client.select((REPLY_TABLE_NAME, reply_id)).await?;
        Ok(data.ok_or(AppError::EntityFailIdNotFound {
            ident: reply_id.to_string(),
        })?)
    }
}
