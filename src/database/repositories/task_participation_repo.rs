use crate::entities::task::task_deliverable_entity::TaskDeliverable;
use crate::entities::task::task_request_participation_entity::TaskRequestParticipation;
use crate::entities::user_auth::local_user_entity;
use crate::entities::wallet::lock_transaction_entity;
use crate::entities::wallet::lock_transaction_entity::LockTransactionDbService;
use crate::entities::wallet::wallet_entity::CurrencySymbol;
use crate::middleware::ctx::Ctx;
use crate::middleware::error::{AppResult, CtxResult};
use crate::{
    database::repository::Repository,
    middleware::error::AppError,
};
use std::collections::HashMap;
use surrealdb::sql::Thing;

const TABLE_COL_USER: &str = local_user_entity::TABLE_NAME;
const LOCK_TABLE_NAME: &str = lock_transaction_entity::TABLE_NAME;

impl Repository<TaskRequestParticipation> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let table_name = self.table_name.as_str();
   let curr_usd = CurrencySymbol::USD.to_string();
        let curr_reef = CurrencySymbol::REEF.to_string();
        let curr_eth = CurrencySymbol::ETH.to_string();
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {table_name} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS user ON TABLE {table_name} TYPE record<{TABLE_COL_USER}>;
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

    pub async fn get_ids(
        &self,
        participant_ids: &Vec<Thing>,
    ) -> CtxResult<Vec<TaskRequestParticipation>> {
        let mut bindings: HashMap<String, String> = HashMap::new();
        let mut ids_str: Vec<String> = vec![];
        participant_ids.into_iter().enumerate().for_each(|i_id| {
            let param_name = format!("id_{}", i_id.0);
            bindings.insert(param_name.clone(), i_id.1.to_raw());
            ids_str.push(format!("<record>${param_name}"));
        });
        let ids_str = ids_str.into_iter().collect::<Vec<String>>().join(",");

        let qry = format!("SELECT * FROM {};", ids_str);
        let query = self.client.query(qry);
        let query = bindings
            .into_iter()
            .fold(query, |query, n_val| query.bind(n_val));
        let mut res = query.await?;
        let res: Vec<TaskRequestParticipation> = res.take(0)?;
        Ok(res)
    }

    pub async fn process_payments(
        &self,
        ctx: &Ctx,
        to_user: &Thing,
        participation_ids: Vec<Thing>,
    ) -> AppResult<()> {
        let participations = self.get_ids(&participation_ids).await?;

        // TODO execute in separate tokio tasks
        for participation in participations {
            if let Some(locked) = participation.lock {
                // not returning on error so successful payments are made
                let pay_locked = LockTransactionDbService {
                    db: self.client.as_ref(),
                    ctx,
                }
                    .process_locked_payment(&locked, &to_user)
                    .await;
                if let Err(err) = pay_locked {
                    // TODO - how to save errors to recover funds later
                    println!("ERR paying task delivery err={:?}", err);
                } else {
                    // println!("PAID {}", locked);
                }
            }
        }
        Ok(())
    }
}