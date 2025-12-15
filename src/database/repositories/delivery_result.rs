use std::sync::Arc;

use surrealdb::engine::any;
use surrealdb::method::Query;
use surrealdb::sql::Thing;

use crate::entities::task_request_user::TaskParticipant;
use crate::interfaces::repositories::delivery_result::DeliveryResultRepositoryInterface;
use crate::middleware::error::AppResult;
use crate::{database::client::Db, middleware::error::AppError};

use super::super::table_names::{DELIVERY_RESULT_TABLE_NAME, TASK_PARTICIPANT_TABLE_NAME};
use crate::entities::community::post_entity::TABLE_NAME as POST_TABLE_NAME;
use crate::entities::wallet::balance_transaction_entity::TABLE_NAME as TRANSACTION_TABLE_NAME;

#[derive(Debug)]
pub struct DeliveryResultRepository {
    client: Arc<Db>,
}

impl DeliveryResultRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self { client }
    }
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("    
    DEFINE TABLE IF NOT EXISTS {DELIVERY_RESULT_TABLE_NAME} TYPE RELATION IN {TASK_PARTICIPANT_TABLE_NAME} OUT {POST_TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
    DEFINE FIELD IF NOT EXISTS reward_tx    ON {DELIVERY_RESULT_TABLE_NAME} FLEXIBLE TYPE option<record<{TRANSACTION_TABLE_NAME}>>;
    DEFINE FIELD IF NOT EXISTS created_at   ON TABLE {DELIVERY_RESULT_TABLE_NAME} TYPE datetime DEFAULT time::now();
    ");
        let mutation = self.client.query(sql).await?;

        mutation
            .check()
            .expect("should mutate DeliveryResultRepository");

        Ok(())
    }
}

#[async_trait::async_trait]
impl DeliveryResultRepositoryInterface for DeliveryResultRepository {
    fn build_create_query<'b>(
        &self,
        query: Query<'b, any::Any>,
        task_participant_id: &str,
        post_id: &str,
        tx_id: Option<&str>,
    ) -> Query<'b, any::Any> {
        query.query( format!("
            LET $delivery_result=RELATE $_delivery_result_task_participant_id->{DELIVERY_RESULT_TABLE_NAME}->$_delivery_result_post_id SET
                tx_id=$_delivery_result_tx_id"))

            .bind(("_delivery_result_task_participant_id", Thing::from((TASK_PARTICIPANT_TABLE_NAME, task_participant_id))))
            .bind(("_delivery_result_post_id", Thing::from((POST_TABLE_NAME, post_id))))
            .bind(("_delivery_result_tx_id", tx_id.map( |id| Thing::from((TRANSACTION_TABLE_NAME, id)))))
    }

    async fn get_by_post(&self, post_id: &str) -> AppResult<TaskParticipant> {
        let mut res = self
            .client
            .query(format!(
                "SELECT in FROM ONLY $post<-{DELIVERY_RESULT_TABLE_NAME};"
            ))
            .bind(("$post", Thing::from((POST_TABLE_NAME, post_id))))
            .await?;

        let data = res.take::<Option<TaskParticipant>>((0, "in"))?;
        Ok(data.ok_or(AppError::EntityFailIdNotFound {
            ident: post_id.to_string(),
        })?)
    }
}
