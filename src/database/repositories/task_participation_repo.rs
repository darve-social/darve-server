use crate::database::client::Db;
use crate::entities::task::task_request_entity::TABLE_NAME as TASK_TABLE_NAME;
use crate::entities::task::task_request_participation_entity::TABLE_NAME;
use crate::entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME;
use crate::entities::wallet::lock_transaction_entity::TABLE_NAME as LOCK_TABLE_NAME;
use crate::entities::wallet::wallet_entity::CurrencySymbol;
use crate::interfaces::repositories::task_participators::TaskParticipatorsRepositoryInterface;
use crate::middleware::error::AppError;
use async_trait::async_trait;
use std::sync::Arc;
use surrealdb::sql::Thing;
#[derive(Debug)]
pub struct TaskRequestParticipatorsRepository {
    client: Arc<Db>,
    table_name: &'static str,
}

impl TaskRequestParticipatorsRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self {
            client,
            table_name: TABLE_NAME,
        }
    }
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let table_name = self.table_name;
        let curr_usd = CurrencySymbol::USD.to_string();
        let curr_reef = CurrencySymbol::REEF.to_string();
        let curr_eth = CurrencySymbol::ETH.to_string();
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {table_name} TYPE RELATION IN {TASK_TABLE_NAME} OUT {USER_TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
    DEFINE FIELD IF NOT EXISTS amount ON TABLE {table_name} TYPE number;
    DEFINE FIELD IF NOT EXISTS lock ON TABLE {table_name} TYPE record<{LOCK_TABLE_NAME}>;
    DEFINE FIELD IF NOT EXISTS votes ON TABLE {table_name} TYPE option<array<{{deliverable_ident: string, points: int}}>>;
    DEFINE FIELD IF NOT EXISTS currency ON TABLE {table_name} TYPE '{curr_usd}'|'{curr_reef}'|'{curr_eth}';
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {table_name} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {table_name} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.client.query(sql).await?;

        mutation
            .check()
            .expect("should mutate taskRequestParticipation");

        Ok(())
    }
}

#[async_trait]
impl TaskParticipatorsRepositoryInterface for TaskRequestParticipatorsRepository {
    async fn create(
        &self,
        task_id: &str,
        user_id: &str,
        tx_id: &str,
        amount: u64,
        currency: &str,
    ) -> Result<String, String> {
        let sql = format!(
            "RELATE $task->{}->$user SET amount=$amount,lock=$tx_id,currency=$currency RETURN record::id(id) as id;",
            self.table_name
        );

        let mut res = self
            .client
            .query(sql)
            .bind(("user", Thing::from((USER_TABLE_NAME, user_id))))
            .bind(("task", Thing::from((TASK_TABLE_NAME, task_id))))
            .bind(("amount", amount))
            .bind(("tx_id", Thing::from((LOCK_TABLE_NAME, tx_id))))
            .bind(("currency", currency.to_string()))
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
        tx_id: &str,
        amount: u64,
        currency: &str,
    ) -> Result<(), String> {
        let query = "UPDATE $id SET amount=$amount,lock=$tx_id,currency=$currency'";
        let res = self
            .client
            .query(query)
            .bind(("id", Thing::from((self.table_name.as_ref(), id))))
            .bind(("amount", amount))
            .bind(("tx_id", Thing::from((LOCK_TABLE_NAME, tx_id))))
            .bind(("currency", currency.to_string()))
            .await
            .map_err(|e| e.to_string())?;
        res.check().map_err(|e| e.to_string())?;

        Ok(())
    }
}
