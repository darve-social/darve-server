use super::super::table_names::TASK_PARTICIPANT_TABLE_NAME;
use crate::database::query_builder::SurrealQueryBuilder;
use crate::database::surrdb_utils::get_thing;
use crate::database::table_names::TASK_REQUEST_TABLE_NAME;
use crate::entities::task_request_user::{TaskParticipant, TaskParticipantResult};
use crate::{
    database::client::Db,
    entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME,
    interfaces::repositories::task_participants::TaskParticipantsRepositoryInterface,
    middleware::{
        error::AppError,
        utils::db_utils::{Pagination, QryOrder},
    },
};
use async_trait::async_trait;
use std::sync::Arc;
use surrealdb::types::RecordId;

#[derive(Debug)]
pub struct TaskParticipantsRepository {
    client: Arc<Db>,
}

impl TaskParticipantsRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self { client }
    }

    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
        DEFINE TABLE IF NOT EXISTS {TASK_PARTICIPANT_TABLE_NAME} TYPE RELATION IN {TASK_REQUEST_TABLE_NAME} OUT {USER_TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
        DEFINE FIELD IF NOT EXISTS timelines    ON {TASK_PARTICIPANT_TABLE_NAME} TYPE array<{{status: string, date: datetime}}>;
        DEFINE FIELD IF NOT EXISTS status       ON {TASK_PARTICIPANT_TABLE_NAME} TYPE string;
        DEFINE FIELD IF NOT EXISTS result       ON {TASK_PARTICIPANT_TABLE_NAME} TYPE option<{{ link: option<string>, post: option<record> }}>;
        DEFINE FIELD IF NOT EXISTS reward_tx   ON {TASK_PARTICIPANT_TABLE_NAME} TYPE option<record>;
        DEFINE INDEX IF NOT EXISTS status_idx   ON {TASK_PARTICIPANT_TABLE_NAME} FIELDS status;
        DEFINE FIELD IF NOT EXISTS r_created    ON TABLE {TASK_PARTICIPANT_TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
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
    fn build_create_query(
        &self,
        query: SurrealQueryBuilder,
        task_id: &str,
        user_ids: Vec<String>,
        status: &str,
    ) -> SurrealQueryBuilder {
        let users = user_ids
            .into_iter()
            .map(|id| RecordId::new(USER_TABLE_NAME, id.as_str()))
            .collect::<Vec<RecordId>>();

        query.query( format!("
            LET $task_participant=RELATE $_task_participant_task_id->{}->$_task_participant_user_ids SET
                timelines=[{{ status: $_task_participant_status, date: time::now() }}],
                status=$_task_participant_status", TASK_PARTICIPANT_TABLE_NAME))

            .bind_var("_task_participant_user_ids", users)
            .bind_var("_task_participant_task_id", get_thing(task_id).expect("Task id invalid"))
            .bind_var("_task_participant_status", status.to_string())
    }

    fn build_update_query(
        &self,
        query: SurrealQueryBuilder,
        id: &str,
        status: &str,
        result: Option<&TaskParticipantResult>,
    ) -> SurrealQueryBuilder {
        query
            .query(format!(
                "
            LET $task_participant=UPDATE $_task_participant_id SET
            timelines+=[{{ status: $_task_participant_status, date: time::now() }}],
            status=$_task_participant_status,
            result=$_task_participant_result;"
            ))
            .bind_var(
                "_task_participant_id",
                RecordId::new(TASK_PARTICIPANT_TABLE_NAME, id),
            )
            .bind_var("_task_participant_status", status.to_string())
            .bind_var("_task_participant_result", result.cloned())
    }

    async fn create(
        &self,
        task_id: &str,
        user_id: &str,
        status: &str,
    ) -> Result<TaskParticipant, String> {
        let sql = format!("
            RELATE $task->{}->$user SET timelines=[{{ status: $status, date: time::now() }}], status=$status
            RETURN record::id(id) AS id, record::id(in) AS task, record::id(out) AS user, status, timelines, result", TASK_PARTICIPANT_TABLE_NAME);

        let mut res = self
            .client
            .query(sql)
            .bind(("user", RecordId::new(USER_TABLE_NAME, user_id)))
            .bind(("task", get_thing(task_id).expect("Task id invalid")))
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
        result: Option<&TaskParticipantResult>,
    ) -> Result<TaskParticipant, String> {
        let query = format!(
            "UPDATE $id SET timelines+=[{{ status: $status, date: time::now() }}], status=$status, result=$result
            RETURN record::id(id) AS id, record::id(in) AS task, record::id(out) AS user, status, timelines, result;"
        );

        let mut res = self
            .client
            .query(query)
            .bind(("id", RecordId::new(TASK_PARTICIPANT_TABLE_NAME, id)))
            .bind(("status", status.to_string()))
            .bind(("result", result.cloned()))
            .await
            .map_err(|e| e.to_string())?;

        let data = res
            .take::<Option<TaskParticipant>>(0)
            .map_err(|e| e.to_string())?;

        Ok(data.unwrap())
    }

    async fn get_by_task(
        &self,
        task_id: &str,
        pagination: Option<Pagination>,
    ) -> Result<Vec<TaskParticipant>, String> {
        let pagination_str = match pagination {
            Some(ref p) => format!(
                "ORDER BY r_created {} LIMIT {} START {}",
                p.order_dir.as_ref().unwrap_or(&QryOrder::ASC),
                p.count,
                p.start
            ),
            None => "".into(),
        };

        let sql = format!(
            "SELECT record::id(id) AS id, record::id(in) AS task, record::id(out) AS user, status, timelines, result FROM {TASK_PARTICIPANT_TABLE_NAME}
            WHERE in = $task {pagination_str};",
        );

        let mut res = self
            .client
            .query(sql)
            .bind(("task", get_thing(task_id).expect("Task id invalid")))
            .await
            .map_err(|e| e.to_string())?;

        let records = res
            .take::<Vec<TaskParticipant>>(0)
            .map_err(|e| e.to_string())?;

        Ok(records)
    }
}
