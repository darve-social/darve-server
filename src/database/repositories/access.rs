use crate::database::client::Db;
use crate::database::table_names::ACCESS_TABLE_NAME;
use crate::entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME;
use crate::interfaces::repositories::access::AccessRepositoryInterface;
use crate::middleware::error::{AppError, AppResult};
use async_trait::async_trait;
use std::sync::Arc;
use surrealdb::sql::Thing;

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
    DEFINE INDEX IF NOT EXISTS in_out_unique_idx ON like FIELDS in, out UNIQUE;
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
    async fn add(&self, users: Vec<Thing>, entities: Vec<Thing>, role: String) -> AppResult<()> {
        let _ = self
            .client
            .query(format!(
                "RELATE $users->{ACCESS_TABLE_NAME}->$entities SET role=$role"
            ))
            .bind(("users", users))
            .bind(("entities", entities))
            .bind(("role", role))
            .await?
            .check();

        Ok(())
    }

    async fn update(&self, user: Thing, entity: Thing, role: String) -> AppResult<()> {
        let _ = self
            .client
            .query(format!(
                "UPDATE $user->{ACCESS_TABLE_NAME} SET role=$role WHERE out = $entity;"
            ))
            .bind(("user", user))
            .bind(("entity", entity))
            .bind(("role", role))
            .await?
            .check();

        Ok(())
    }

    async fn remove_by_user(&self, user: Thing, entities: Vec<Thing>) -> AppResult<()> {
        let _ = self
            .client
            .query(format!(
                "DELETE $user->{ACCESS_TABLE_NAME} WHERE out IN $entities; "
            ))
            .bind(("user", user))
            .bind(("entities", entities))
            .await?
            .check();

        Ok(())
    }

    async fn remove_by_entity(&self, entity: Thing, users: Vec<Thing>) -> AppResult<()> {
        let _ = self
            .client
            .query(format!(
                "DELETE $entity<-{ACCESS_TABLE_NAME} WHERE in IN $users; "
            ))
            .bind(("users", users))
            .bind(("entity", entity))
            .await?
            .check();

        Ok(())
    }
}
