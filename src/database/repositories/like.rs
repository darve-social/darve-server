use crate::database::client::Db;
use crate::database::table_names::LIKE_TABLE_NAME;
use crate::entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME;
use crate::interfaces::repositories::like::LikesRepositoryInterface;
use crate::middleware::error::{AppError, AppResult};
use async_trait::async_trait;
use std::sync::Arc;
use surrealdb::types::RecordId;

#[derive(Debug)]
pub struct LikesRepository {
    client: Arc<Db>,
}

impl LikesRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self { client }
    }
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("

    DEFINE TABLE IF NOT EXISTS {LIKE_TABLE_NAME} TYPE RELATION IN {USER_TABLE_NAME} OUT post|reply ENFORCED SCHEMAFULL PERMISSIONS NONE;
    DEFINE INDEX IF NOT EXISTS in_out_unique_idx ON {LIKE_TABLE_NAME} FIELDS in, out UNIQUE;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE {LIKE_TABLE_NAME} TYPE datetime DEFAULT time::now();
    DEFINE FIELD IF NOT EXISTS count ON TABLE {LIKE_TABLE_NAME} TYPE number;

    ");
        let mutation = self.client.query(sql).await?;

        mutation.check().expect("should mutate LikesRepository");

        Ok(())
    }
}

#[async_trait]
impl LikesRepositoryInterface for LikesRepository {
    async fn like(&self, user: RecordId, out: RecordId, count: u16) -> AppResult<u32> {
        let mut res = self
            .client
            .query(format!(
                "BEGIN TRANSACTION; \
                LET $id = (SELECT id FROM {LIKE_TABLE_NAME} WHERE in=$in AND out=$out)[0].id; \
                IF $id THEN UPDATE $id SET count=$count ELSE RELATE $in->{LIKE_TABLE_NAME}->$out SET count=$count END; \
                LET $count = math::sum((SELECT VALUE count FROM {LIKE_TABLE_NAME} WHERE out=$out)); \
                UPDATE $out SET likes_nr=$count; \
                COMMIT TRANSACTION; \
                RETURN $count;"
            ))
            .bind(("in", user))
            .bind(("out", out))
            .bind(("count", count))
            .await?;

        let count = res.take::<Option<i64>>(res.num_statements() - 1)?.unwrap_or(0) as u32;
        Ok(count)
    }

    async fn unlike(&self, user: RecordId, out: RecordId) -> AppResult<u32> {
        let mut res = self
            .client
            .query(format!(
                "BEGIN TRANSACTION; \
                DELETE {LIKE_TABLE_NAME} WHERE in=$in AND out=$out; \
                LET $count = math::sum((SELECT VALUE count FROM {LIKE_TABLE_NAME} WHERE out=$out)); \
                UPDATE $out SET likes_nr=$count; \
                COMMIT TRANSACTION; \
                RETURN $count;"
            ))
            .bind(("in", user))
            .bind(("out", out))
            .await?;

        let count = res.take::<Option<i64>>(res.num_statements() - 1)?.unwrap_or(0) as u32;
        Ok(count)
    }
}
