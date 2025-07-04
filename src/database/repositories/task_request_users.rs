use crate::{
    database::client::Db,
    entities::task_request_user::{TaskRequestUserResult, TaskRequestUserTimeline},
    interfaces::repositories::task_request_users::TaskRequestUsersRepositoryInterface,
    middleware::error::AppError,
};
use async_trait::async_trait;
use std::sync::Arc;
use surrealdb::sql::Thing;

#[derive(Debug)]
pub struct TaskRequestUsesRepository {
    client: Arc<Db>,
}

impl TaskRequestUsesRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self { client }
    }

    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = "
        DEFINE TABLE IF NOT EXISTS task_request_user TYPE RELATION IN task_request OUT local_user ENFORCED SCHEMAFULL PERMISSIONS NONE;
        DEFINE FIELD IF NOT EXISTS timelines    ON task_request_user FLEXIBLE TYPE array<object>;
        DEFINE FIELD IF NOT EXISTS result       ON task_request_user FLEXIBLE TYPE option<object>;
        DEFINE FIELD IF NOT EXISTS created_at   ON task_request_user TYPE datetime DEFAULT time::now();
        DEFINE INDEX IF NOT EXISTS in_out_idx   ON task_request_user FIELDS in, out;
    ";
        let mutation = self.client.query(sql).await?;

        mutation
            .check()
            .expect("should mutate TaskRequestUsesRepository");

        Ok(())
    }
}

#[async_trait]
impl TaskRequestUsersRepositoryInterface for TaskRequestUsesRepository {
    async fn create(
        &self,
        task_id: &str,
        user_id: &str,
        timeline: Option<TaskRequestUserTimeline>,
    ) -> Result<String, String> {
        let timelines = match timeline {
            Some(t) => vec![t],
            _ => Vec::new(),
        };
        let sql = "
            RELATE $task->task_request_user->$user SET timelines=$timelines 
            RETURN record::id(id) as id;";

        let mut res = self
            .client
            .query(sql)
            .bind(("user", Thing::from(("local_user", user_id))))
            .bind(("task", Thing::from(("task_request", task_id))))
            .bind(("timelines", timelines))
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
        timeline: TaskRequestUserTimeline,
        result: Option<TaskRequestUserResult>,
    ) -> Result<(), String> {
        let res = if result.is_some() {
            ",result=$result"
        } else {
            ""
        };
        let query = format!("UPDATE $id SET timelines+=[$timeline] {};", res);
        let res = self
            .client
            .query(query)
            .bind(("id", Thing::from(("task_request_user", id))))
            .bind(("timeline", timeline))
            .bind(("result", result))
            .await
            .map_err(|e| e.to_string())?;

        res.check().map_err(|e| e.to_string())?;

        Ok(())
    }
}
