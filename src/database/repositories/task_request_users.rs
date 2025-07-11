use crate::{
    database::client::Db,
    entities::{
        task::task_request_entity::TABLE_NAME as TASK_TABLE_NAME,
        task_request_user::TaskRequestUserResult,
        user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME,
    },
    interfaces::repositories::task_request_users::TaskRequestUsersRepositoryInterface,
    middleware::error::AppError,
};
use async_trait::async_trait;
use std::sync::Arc;
use surrealdb::sql::Thing;

#[derive(Debug)]
pub struct TaskRequestUsesRepository {
    client: Arc<Db>,
    table_name: &'static str,
}

impl TaskRequestUsesRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self {
            client,
            table_name: "task_request_user",
        }
    }

    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let table = self.table_name;
        let sql = format!("
        DEFINE TABLE IF NOT EXISTS {table} TYPE RELATION IN {TASK_TABLE_NAME} OUT {USER_TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
        DEFINE FIELD IF NOT EXISTS timelines    ON {table} TYPE array<{{status: string, date: datetime}}>;
        DEFINE FIELD IF NOT EXISTS status       ON {table} TYPE string;
        DEFINE FIELD IF NOT EXISTS result       ON {table} FLEXIBLE TYPE option<object>;
        DEFINE INDEX IF NOT EXISTS in_out_idx   ON {table} FIELDS in, out;
        DEFINE INDEX IF NOT EXISTS status_idx   ON {table} FIELDS status;
    ");
        let mutation = self.client.query(sql).await?;

        mutation
            .check()
            .expect("should mutate TaskRequestUsesRepository");

        Ok(())
    }
}

#[async_trait]
impl TaskRequestUsersRepositoryInterface for TaskRequestUsesRepository {
    async fn create(&self, task_id: &str, user_id: &str, status: &str) -> Result<String, String> {
        let sql = format!("
            RELATE $task->{}->$user SET timelines=[{{ status: $status, date: time::now() }}], status=$status
            RETURN record::id(id) as id;", self.table_name);

        let mut res = self
            .client
            .query(sql)
            .bind(("user", Thing::from((USER_TABLE_NAME, user_id))))
            .bind(("task", Thing::from((TASK_TABLE_NAME, task_id))))
            .bind(("status", status.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        let record = res
            .take::<Option<String>>((0, "id"))
            .map_err(|e| e.to_string())?;

        Ok(record.unwrap())
    }

    async fn update(
        &self,
        id: &str,
        status: &str,
        result: Option<TaskRequestUserResult>,
    ) -> Result<(), String> {
        let query = format!(
            "UPDATE $id SET timelines+=[{{ status: $status, date: time::now() }}], status=$status {};",
            if result.is_some() {
                ",result=$result"
            } else {
                ""
            }
        );
        let res = self
            .client
            .query(query)
            .bind(("id", Thing::from((self.table_name, id))))
            .bind(("status", status.to_string()))
            .bind(("result", result))
            .await
            .map_err(|e| e.to_string())?;

        res.check().map_err(|e| e.to_string())?;

        Ok(())
    }
}
