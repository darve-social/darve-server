use crate::database::client::Db;
use crate::entities::task_donor::TaskDonor;
use crate::entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME;
use crate::entities::wallet::balance_transaction_entity::TABLE_NAME as TRANSACTION_TABLE_NAME;
use crate::entities::wallet::wallet_entity::CurrencySymbol;
use crate::interfaces::repositories::task_donors::TaskDonorsRepositoryInterface;
use crate::middleware::error::AppError;
use async_trait::async_trait;
use std::sync::Arc;
use surrealdb::engine::any;
use surrealdb::method::Query;
use surrealdb::sql::Thing;
use crate::database::repositories::task_request_repo::TASK_REQUEST_TABLE_NAME;

#[derive(Debug)]
pub struct TaskDonorsRepository {
    client: Arc<Db>,
    table_name: &'static str,
}

impl TaskDonorsRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self {
            client,
            table_name: "task_donor",
        }
    }
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let table_name = self.table_name;
        let curr_usd = CurrencySymbol::USD.to_string();
        let curr_reef = CurrencySymbol::REEF.to_string();
        let curr_eth = CurrencySymbol::ETH.to_string();
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {table_name} TYPE RELATION IN {TASK_REQUEST_TABLE_NAME} OUT {USER_TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
    DEFINE FIELD IF NOT EXISTS amount ON TABLE {table_name} TYPE number;
    DEFINE FIELD IF NOT EXISTS transaction ON TABLE {table_name} TYPE record<{TRANSACTION_TABLE_NAME}>;
    DEFINE FIELD IF NOT EXISTS votes ON TABLE {table_name} TYPE option<array<{{deliverable_ident: string, points: int}}>>;
    DEFINE FIELD IF NOT EXISTS currency ON TABLE {table_name} TYPE '{curr_usd}'|'{curr_reef}'|'{curr_eth}';
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {table_name} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {table_name} TYPE datetime DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.client.query(sql).await?;

        mutation
            .check()
            .expect("should mutate taskDonorParticipation");

        Ok(())
    }
}

#[async_trait]
impl TaskDonorsRepositoryInterface for TaskDonorsRepository {
    fn build_create_query<'b>(
        &self,
        query: Query<'b, any::Any>,
        task_id: &str,
        user_id: &str,
        tx_id: &str,
        amount: u64,
        currency: &str,
    ) -> Query<'b, any::Any> {
        query
            .query(format!(
                "LET $task_donor=RELATE $_task_donor_task_id->{}->$_task_donor_user_id SET
                amount=$_task_donor_amount,
                transaction={tx_id},
                currency=$_task_donor_currency;",
                self.table_name
            ))
            .bind((
                "_task_donor_user_id",
                Thing::from((USER_TABLE_NAME, user_id)),
            ))
            .bind((
                "_task_donor_task_id",
                Thing::from((TASK_REQUEST_TABLE_NAME, task_id)),
            ))
            .bind(("_task_donor_amount", amount))
            .bind(("_task_donor_currency", currency.to_string()))
    }

    fn build_update_query<'b>(
        &self,
        query: Query<'b, any::Any>,
        id: &str,
        tx_id: &str,
        amount: u64,
        currency: &str,
    ) -> Query<'b, any::Any> {
        query
            .query(format!(
                "LET $task_donor=UPDATE $_task_donor_id SET
                amount=$_task_donor_amount,
                transaction={tx_id},
                currency=$_task_donor_currency;"
            ))
            .bind((
                "_task_donor_id",
                Thing::from((self.table_name.as_ref(), id)),
            ))
            .bind(("_task_donor_amount", amount))
            .bind(("_task_donor_currency", currency.to_string()))
    }

    async fn create(
        &self,
        task_id: &str,
        user_id: &str,
        tx_id: &str,
        amount: u64,
        currency: &str,
    ) -> Result<TaskDonor, String> {
        let sql = format!(
            "RELATE $task->{}->$user SET amount=$amount,transaction=$tx_id,currency=$currency",
            self.table_name
        );

        let mut res = self
            .client
            .query(sql)
            .bind(("user", Thing::from((USER_TABLE_NAME, user_id))))
            .bind(("task", Thing::from((TASK_REQUEST_TABLE_NAME, task_id))))
            .bind(("amount", amount))
            .bind(("tx_id", Thing::from((TRANSACTION_TABLE_NAME, tx_id))))
            .bind(("currency", currency.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        let record = res
            .take::<Option<TaskDonor>>(0)
            .map_err(|e| e.to_string())?;

        Ok(record.unwrap())
    }

    async fn update(
        &self,
        id: &str,
        tx_id: &str,
        amount: u64,
        currency: &str,
    ) -> Result<TaskDonor, String> {
        let query = "UPDATE $id SET amount=$amount,transaction=$tx_id,currency=$currency;";
        let mut res = self
            .client
            .query(query)
            .bind(("id", Thing::from((self.table_name.as_ref(), id))))
            .bind(("amount", amount))
            .bind(("tx_id", Thing::from((TRANSACTION_TABLE_NAME, tx_id))))
            .bind(("currency", currency.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        let record = res
            .take::<Option<TaskDonor>>(0)
            .map_err(|e| e.to_string())?;

        Ok(record.unwrap())
    }
}
