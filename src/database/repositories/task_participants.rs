use crate::{
    database::client::Db,
    entities::{
        task::task_request_entity::TABLE_NAME as TASK_TABLE_NAME,
        task_request_user::{TaskParticipant, TaskParticipantResult},
        user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME,
        wallet::balance_transaction_entity::TABLE_NAME as TRANSACTION_TABLE_NAME,
    },
    interfaces::repositories::task_participants::TaskParticipantsRepositoryInterface,
    middleware::error::AppError,
};
use async_trait::async_trait;
use std::sync::Arc;
use surrealdb::{engine::any, method::Query, sql::Thing};

#[derive(Debug)]
pub struct TaskParticipantsRepository {
    client: Arc<Db>,
    table_name: &'static str,
}

impl TaskParticipantsRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self {
            client,
            table_name: "task_participant",
        }
    }

    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let table_name = self.table_name;
        let sql = format!("
        DEFINE TABLE IF NOT EXISTS {table_name} TYPE RELATION IN {TASK_TABLE_NAME} OUT {USER_TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
        DEFINE FIELD IF NOT EXISTS timelines    ON {table_name} TYPE array<{{status: string, date: datetime}}>;
        DEFINE FIELD IF NOT EXISTS status       ON {table_name} TYPE string;
        DEFINE FIELD IF NOT EXISTS result       ON {table_name} FLEXIBLE TYPE option<object>;
        DEFINE FIELD IF NOT EXISTS reward_tx    ON {table_name} FLEXIBLE TYPE option<record<{TRANSACTION_TABLE_NAME}>>;
        DEFINE INDEX IF NOT EXISTS status_idx   ON {table_name} FIELDS status;
        DEFINE FIELD IF NOT EXISTS r_created ON TABLE {table_name} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
    ");
        let mutation = self.client.query(sql).await?;

        mutation
            .check()
            .expect("should mutate TaskParticipantsRepository");

        Ok(())
    }
}

#[async_trait]
impl TaskParticipantsRepositoryInterface for TaskParticipantsRepository {
    fn build_create_query<'b>(
        &self,
        query: Query<'b, any::Any>,
        task_id: &str,
        user_ids: Vec<String>,
        status: &str,
    ) -> Query<'b, any::Any> {
        let users = user_ids
            .into_iter()
            .map(|id| Thing::from((USER_TABLE_NAME, id.as_str())))
            .collect::<Vec<Thing>>();

        query.query( format!("
            LET $task_participant=RELATE $_task_participant_task_id->{}->$_task_participant_user_ids SET
                timelines=[{{ status: $_task_participant_status, date: time::now() }}],
                status=$_task_participant_status", self.table_name))

            .bind(("_task_participant_user_ids", users))
            .bind(("_task_participant_task_id", Thing::from((TASK_TABLE_NAME, task_id))))
            .bind(("_task_participant_status", status.to_string()))
    }

    fn build_update_query<'b>(
        &self,
        query: Query<'b, any::Any>,
        id: &str,
        status: &str,
        result: Option<TaskParticipantResult>,
    ) -> Query<'b, any::Any> {
        let update_result = match result.is_some() {
            true => ",result=$_task_participant_result",
            false => "",
        };
        query
            .query(format!(
                "
            LET $task_participant=UPDATE $_task_participant_id SET
            timelines+=[{{ status: $_task_participant_status, date: time::now() }}],
            status=$_task_participant_status {update_result};"
            ))
            .bind(("_task_participant_id", Thing::from((self.table_name, id))))
            .bind(("_task_participant_status", status.to_string()))
            .bind(("_task_participant_result", result))
    }

    async fn create(
        &self,
        task_id: &str,
        user_id: &str,
        status: &str,
    ) -> Result<TaskParticipant, String> {
        let sql = format!("
            RELATE $task->{}->$user SET timelines=[{{ status: $status, date: time::now() }}], status=$status", self.table_name);

        let mut res = self
            .client
            .query(sql)
            .bind(("user", Thing::from((USER_TABLE_NAME, user_id))))
            .bind(("task", Thing::from((TASK_TABLE_NAME, task_id))))
            .bind(("status", status.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        let record = res
            .take::<Option<TaskParticipant>>(0)
            .map_err(|e| e.to_string())?;

        Ok(record.unwrap())
    }

    async fn update(
        &self,
        id: &str,
        status: &str,
        result: Option<TaskParticipantResult>,
    ) -> Result<TaskParticipant, String> {
        let update_result = match result.is_some() {
            true => ",result=$result",
            false => "",
        };
        let query = format!(
            "UPDATE $id SET timelines+=[{{ status: $status, date: time::now() }}], status=$status {update_result};");

        let mut res = self
            .client
            .query(query)
            .bind(("id", Thing::from((self.table_name, id))))
            .bind(("status", status.to_string()))
            .bind(("result", result))
            .await
            .map_err(|e| e.to_string())?;

        let data = res
            .take::<Option<TaskParticipant>>(0)
            .map_err(|e| e.to_string())?;

        Ok(data.unwrap())
    }
}
