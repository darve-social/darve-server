use crate::database::client::Db;
use crate::database::surrdb_utils::get_thing;
use crate::database::table_names::ACCESS_TABLE_NAME;
use crate::entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME;
use crate::interfaces::repositories::access::AccessRepositoryInterface;
use crate::middleware::error::{AppError, AppResult};
use async_trait::async_trait;
use std::sync::Arc;
use surrealdb::types::RecordId;

#[derive(Debug)]
pub struct AccessRepository {
    client: Arc<Db>,
}

impl AccessRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self { client }
    }
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("

    DEFINE TABLE IF NOT EXISTS {ACCESS_TABLE_NAME} TYPE RELATION IN {USER_TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
    DEFINE INDEX IF NOT EXISTS in_out_unique_idx ON {ACCESS_TABLE_NAME} FIELDS in, out UNIQUE;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE {ACCESS_TABLE_NAME} TYPE datetime DEFAULT time::now();
    DEFINE FIELD IF NOT EXISTS role ON TABLE {ACCESS_TABLE_NAME} TYPE string;
    DEFINE INDEX IF NOT EXISTS idx_role ON TABLE {ACCESS_TABLE_NAME} COLUMNS role;

    ");
        let mutation = self.client.query(sql).await?;

        mutation.check().expect("should mutate AccessRepository");

        Ok(())
    }
}

#[async_trait]
impl AccessRepositoryInterface for AccessRepository {
    async fn add(&self, users: Vec<RecordId>, entities: Vec<&str>, role: String) -> AppResult<()> {
        let mut things = Vec::with_capacity(entities.len());
        for id in entities {
            things.push(get_thing(id).map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?);
        }
        let _ = self
            .client
            .query(format!(
                "RELATE $users->{ACCESS_TABLE_NAME}->$entities SET role=$role"
            ))
            .bind(("users", users))
            .bind(("entities", things))
            .bind(("role", role))
            .await?
            .check();

        Ok(())
    }

    async fn update(&self, user: RecordId, entity: &str, role: String) -> AppResult<()> {
        let thing = get_thing(entity).map_err(|e| AppError::SurrealDb {
            source: e.to_string(),
        })?;
        let _ = self
            .client
            .query(format!(
                "UPDATE $user->{ACCESS_TABLE_NAME} SET role=$role WHERE out = $entity;"
            ))
            .bind(("user", user))
            .bind(("entity", thing))
            .bind(("role", role))
            .await?
            .check();

        Ok(())
    }

    async fn remove_by_user(&self, user: RecordId, entities: Vec<&str>) -> AppResult<()> {
        let mut things = Vec::with_capacity(entities.len());
        for id in entities {
            things.push(get_thing(id).map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?);
        }
        let _ = self
            .client
            .query(format!(
                "DELETE $user->{ACCESS_TABLE_NAME} WHERE out IN $entities; "
            ))
            .bind(("user", user))
            .bind(("entities", things))
            .await?
            .check();

        Ok(())
    }

    async fn remove_by_entity(&self, entity: &str, users: Vec<RecordId>) -> AppResult<()> {
        let thing = get_thing(entity).map_err(|e| AppError::SurrealDb {
            source: e.to_string(),
        })?;
        let _ = self
            .client
            .query(format!(
                "DELETE $entity<-{ACCESS_TABLE_NAME} WHERE in IN $users; "
            ))
            .bind(("users", users))
            .bind(("entity", thing))
            .await?
            .check();

        Ok(())
    }
}
