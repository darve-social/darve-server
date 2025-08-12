use crate::database::client::Db;
use crate::entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME;
use crate::interfaces::repositories::like::LikesRepositoryInterface;
use crate::middleware::error::{AppError, AppResult};
use async_trait::async_trait;
use std::sync::Arc;
use surrealdb::sql::Thing;

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
    
    DEFINE TABLE IF NOT EXISTS like TYPE RELATION IN {USER_TABLE_NAME} OUT post|reply ENFORCED SCHEMAFULL PERMISSIONS NONE;
    DEFINE INDEX IF NOT EXISTS in_out_unique_idx ON like FIELDS in, out UNIQUE;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE like TYPE datetime DEFAULT time::now();
    DEFINE FIELD IF NOT EXISTS count ON TABLE like TYPE number;

    ");
        let mutation = self.client.query(sql).await?;

        mutation.check().expect("should mutate LikesRepository");

        Ok(())
    }
}

#[async_trait]
impl LikesRepositoryInterface for LikesRepository {
    async fn like(&self, user: Thing, out: Thing, count: u16) -> AppResult<u32> {
        let mut res = self
            .client
            .query("BEGIN TRANSACTION;")
            .query("LET $id = (SELECT id FROM $in->like WHERE out = $out)[0].id")
            .query(
                "IF $id THEN
                    UPDATE $id SET count=$count
                 ELSE
                    RELATE $in->like->$out SET count=$count
                 END;",
            )
            .query("LET $count = math::sum(SELECT VALUE <-like.count[0] ?? 0 FROM $out);")
            .query("UPDATE $out SET likes_nr=$count;")
            .query("RETURN $count;")
            .query("COMMIT TRANSACTION;")
            .bind(("in", user))
            .bind(("out", out))
            .bind(("count", count))
            .await?;

        let count = res.take::<Option<i64>>(0)?.unwrap() as u32;
        Ok(count)
    }

    async fn unlike(&self, user: Thing, out: Thing) -> AppResult<u32> {
        let mut res = self
            .client
            .query("BEGIN TRANSACTION;")
            .query("DELETE $in->like WHERE out=$out;")
            .query("LET $count = math::sum(SELECT VALUE <-like.count[0] ?? 0 FROM $out);")
            .query("UPDATE $out SET likes_nr=$count;")
            .query("RETURN $count;")
            .query("COMMIT TRANSACTION;")
            .bind(("in", user))
            .bind(("out", out))
            .await?;

        let count = res.take::<Option<i64>>(0)?.unwrap() as u32;
        Ok(count)
    }
}
