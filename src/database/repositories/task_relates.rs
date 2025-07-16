use crate::entities::task::task_request_entity::TABLE_NAME as TASK_TABLE_NAME;
use crate::interfaces::repositories::task_relates::TaskRelatesRepositoryInterface;
use crate::middleware::utils::db_utils::ViewRelateField;
use crate::{
    database::client::Db,
    middleware::error::{AppError, AppResult},
};
use serde::Deserialize;
use std::fmt::Debug;
use std::sync::Arc;
use surrealdb::sql::Thing;

#[derive(Debug)]
pub struct TaskRelatesRepository {
    client: Arc<Db>,
    table_name: &'static str,
}

impl TaskRelatesRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self {
            client,
            table_name: "task_relate",
        }
    }

    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let table_name = self.table_name;
        let sql = format!("
        DEFINE TABLE IF NOT EXISTS {table_name} TYPE RELATION IN {TASK_TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
    ");
        let mutation = self.client.query(sql).await?;

        mutation
            .check()
            .expect("should mutate TaskRelatesRepository");

        Ok(())
    }

    pub async fn get_tasks_by_id<T: for<'de> Deserialize<'de> + ViewRelateField + Debug>(
        &self,
        relate_to: &Thing,
    ) -> AppResult<Vec<T>> {
        let fields = T::get_fields();
        let table_name = self.table_name;
        let mut res = self
            .client
            .query(format!(
                "SELECT <-{table_name}<-{TASK_TABLE_NAME}.{{{fields}}} AS tasks FROM $relate_id;"
            ))
            .bind(("relate_id", relate_to.clone()))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;

        let tasks = res
            .take::<Option<Vec<T>>>("tasks")
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            });

        Ok(tasks?.unwrap_or_default())
    }
}

#[async_trait::async_trait]
impl TaskRelatesRepositoryInterface for TaskRelatesRepository {
    async fn create(&self, task_id: &Thing, relate_to: &Thing) -> AppResult<()> {
        let res = self
            .client
            .query("RELATE $task->task_relate->$relate_id;")
            .bind(("task", task_id.clone()))
            .bind(("relate_id", relate_to.clone()))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;

        res.check().expect("should create task link");
        Ok(())
    }
}
