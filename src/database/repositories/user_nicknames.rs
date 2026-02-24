use crate::database::client::Db;
use crate::database::table_names::NICKNAME_TABLE_NAME;
use crate::entities::nickname::Nickname;
use crate::entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME;
use crate::interfaces::repositories::nickname::NicknamesRepositoryInterface;
use crate::middleware::error::{AppError, AppResult};
use async_trait::async_trait;
use std::sync::Arc;
use surrealdb::types::RecordId;

#[derive(Debug)]
pub struct NicknamesRepository {
    client: Arc<Db>,
}

impl NicknamesRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self { client }
    }
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    
    DEFINE TABLE IF NOT EXISTS {NICKNAME_TABLE_NAME} TYPE RELATION IN {USER_TABLE_NAME} OUT {USER_TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
    DEFINE INDEX IF NOT EXISTS in_out_unique_idx ON {NICKNAME_TABLE_NAME} FIELDS in, out UNIQUE;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE {NICKNAME_TABLE_NAME} TYPE datetime DEFAULT time::now();
    DEFINE FIELD IF NOT EXISTS name ON TABLE {NICKNAME_TABLE_NAME} TYPE string;

    ");
        let mutation = self.client.query(sql).await?;

        mutation.check().expect("should mutate NicknamesRepository");

        Ok(())
    }
}

#[async_trait]
impl NicknamesRepositoryInterface for NicknamesRepository {
    async fn upsert(&self, user_id: &str, to_user_id: &str, name: String) -> AppResult<()> {
        let _ = self
            .client
            .query(format!(
                "LET $res=(UPDATE $in->{NICKNAME_TABLE_NAME} SET name=$name WHERE out=$out)[0].id; \
                IF $res = NONE THEN RELATE $in->{NICKNAME_TABLE_NAME}->$out SET name=$name END;"
            ))
            .bind(("in", RecordId::new(USER_TABLE_NAME, user_id)))
            .bind(("out", RecordId::new(USER_TABLE_NAME, to_user_id)))
            .bind(("name", name))
            .await?;
        Ok(())
    }

    async fn remove(&self, user_id: &str, to_user_id: &str) -> AppResult<()> {
        let _ = self
            .client
            .query(format!("DELETE $in->{NICKNAME_TABLE_NAME} WHERE out=$out;"))
            .bind(("in", RecordId::new(USER_TABLE_NAME, user_id)))
            .bind(("out", RecordId::new(USER_TABLE_NAME, to_user_id)))
            .await?;

        Ok(())
    }

    async fn get_by_user(&self, user_id: &str) -> AppResult<Vec<Nickname>> {
        let mut res = self
            .client
            .query(format!("SELECT record::id(out) AS user_id, name FROM {NICKNAME_TABLE_NAME} WHERE in=$user_in"))
            .bind(("user_in", RecordId::new(USER_TABLE_NAME, user_id)))
            .await?;

        let data: Vec<Nickname> = res.take(0).map_err(|e| AppError::SurrealDb { source: e.to_string() })?;

        Ok(data)
    }
}
