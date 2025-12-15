use super::super::table_names::{DELIVERY_RESULT_TABLE_NAME, TASK_PARTICIPANT_TABLE_NAME};
use crate::{
    database::client::Db,
    entities::{
        task::task_request_entity::TABLE_NAME as TASK_TABLE_NAME,
        task_request_user::TaskParticipant,
        user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME,
    },
    interfaces::repositories::task_participants::TaskParticipantsRepositoryInterface,
    middleware::{
        error::AppError,
        utils::db_utils::{Pagination, QryOrder},
    },
};
use async_trait::async_trait;
use std::sync::Arc;
use surrealdb::{engine::any, method::Query, sql::Thing};
use crate::database::repositories::task_request_repo::TASK_REQUEST_TABLE_NAME;

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
        DEFINE TABLE IF NOT EXISTS {TASK_PARTICIPANT_TABLE_NAME} TYPE RELATION IN {TASK_TABLE_NAME} OUT {USER_TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
        DEFINE FIELD IF NOT EXISTS timelines    ON {TASK_PARTICIPANT_TABLE_NAME} TYPE array<{{status: string, date: datetime}}>;
        DEFINE FIELD IF NOT EXISTS status       ON {TASK_PARTICIPANT_TABLE_NAME} TYPE string;
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
                status=$_task_participant_status", TASK_PARTICIPANT_TABLE_NAME))

            .bind(("_task_participant_user_ids", users))
            .bind(("_task_participant_task_id", Thing::from((TASK_REQUEST_TABLE_NAME, task_id))))
            .bind(("_task_participant_status", status.to_string()))
    }

    fn build_update_query<'b>(
        &self,
        query: Query<'b, any::Any>,
        id: &str,
        status: &str,
    ) -> Query<'b, any::Any> {
        query
            .query(format!(
                "
            LET $task_participant=UPDATE $_task_participant_id SET
            timelines+=[{{ status: $_task_participant_status, date: time::now() }}],
            status=$_task_participant_status;"
            ))
            .bind((
                "_task_participant_id",
                Thing::from((TASK_PARTICIPANT_TABLE_NAME, id)),
            ))
            .bind(("_task_participant_status", status.to_string()))
    }

    async fn create(
        &self,
        task_id: &str,
        user_id: &str,
        status: &str,
    ) -> Result<TaskParticipant, String> {
        let sql = format!("
            RELATE $task->{}->$user SET timelines=[{{ status: $status, date: time::now() }}], status=$status", TASK_PARTICIPANT_TABLE_NAME);

        let mut res = self
            .client
            .query(sql)
            .bind(("user", Thing::from((USER_TABLE_NAME, user_id))))
            .bind(("task", Thing::from((TASK_REQUEST_TABLE_NAME, task_id))))
            .bind(("status", status.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        let record = res
            .take::<Option<TaskParticipant>>(0)
            .map_err(|e| e.to_string())?;

        Ok(record.unwrap())
    }

    async fn update(&self, id: &str, status: &str) -> Result<TaskParticipant, String> {
        let query = format!(
            "UPDATE $id SET timelines+=[{{ status: $status, date: time::now() }}], status=$status;"
        );

        let mut res = self
            .client
            .query(query)
            .bind(("id", Thing::from((TASK_PARTICIPANT_TABLE_NAME, id))))
            .bind(("status", status.to_string()))
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
                "ORDER BY created_at {} LIMIT {} START {}",
                p.order_dir.as_ref().unwrap_or(&QryOrder::ASC),
                p.count,
                p.start
            ),
            None => "".into(),
        };

        let sql = format!(
            "SELECT *, ->{DELIVERY_RESULT_TABLE_NAME}.out as delivery_post FROM {TASK_PARTICIPANT_TABLE_NAME} 
            WHERE task = $task {pagination_str};",
        );

        let mut res = self
            .client
            .query(sql)
            .bind(("task", Thing::from((TASK_TABLE_NAME, task_id))))
            .await
            .map_err(|e| e.to_string())?;

        let records = res
            .take::<Vec<TaskParticipant>>(0)
            .map_err(|e| e.to_string())?;

        Ok(records)
    }
}
