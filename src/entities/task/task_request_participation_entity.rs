use middleware::error::AppResult;
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::sql::{Id, Thing};
use wallet::lock_transaction_entity::LockTransactionDbService;
use wallet::wallet_entity::CurrencySymbol;

use crate::database::client::Db;
use crate::entities::user_auth::local_user_entity;
use crate::entities::wallet::{self, lock_transaction_entity};
use crate::middleware;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequestParticipantion {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub(crate) amount: i64,
    pub(crate) currency: CurrencySymbol,
    pub(crate) user: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) lock: Option<Thing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) votes: Option<Vec<RewardVote>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RewardVote {
    deliverable_ident: String,
    points: i32,
}

pub struct TaskParticipationDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "task_request_participation";
const TABLE_COL_USER: &str = local_user_entity::TABLE_NAME;
// const TABLE_COL_TASK_REQUEST: &str = crate::entity::task_request_entitiy::TABLE_NAME;
const LOCK_TABLE_NAME: &str = lock_transaction_entity::TABLE_NAME;

impl<'a> TaskParticipationDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let curr_usd = CurrencySymbol::USD.to_string();
        let curr_reef = CurrencySymbol::REEF.to_string();
        let curr_eth = CurrencySymbol::ETH.to_string();
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS user ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE FIELD IF NOT EXISTS amount ON TABLE {TABLE_NAME} TYPE number;
    DEFINE FIELD IF NOT EXISTS lock ON TABLE {TABLE_NAME} TYPE record<{LOCK_TABLE_NAME}>;
    DEFINE FIELD IF NOT EXISTS votes ON TABLE {TABLE_NAME} TYPE option<array<{{deliverable_ident: string, points: int}}>>;
    DEFINE FIELD IF NOT EXISTS currency ON TABLE {TABLE_NAME} TYPE '{curr_usd}'|'{curr_reef}'|'{curr_eth}';
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.db.query(sql).await?;

        mutation
            .check()
            .expect("should mutate taskRequestParticipation");

        Ok(())
    }

    pub async fn create_update(
        &self,
        record: TaskRequestParticipantion,
    ) -> CtxResult<TaskRequestParticipantion> {
        let resource = record
            .id
            .clone()
            .unwrap_or(Thing::from((TABLE_NAME.to_string(), Id::rand())));

        self.db
            .upsert((resource.tb, resource.id.to_raw()))
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<TaskRequestParticipantion>| v.unwrap())
    }

    pub async fn get_ids(
        &self,
        participant_ids: &Vec<Thing>,
    ) -> CtxResult<Vec<TaskRequestParticipantion>> {
        let mut bindings: HashMap<String, String> = HashMap::new();
        let mut ids_str: Vec<String> = vec![];
        participant_ids.into_iter().enumerate().for_each(|i_id| {
            let param_name = format!("id_{}", i_id.0);
            bindings.insert(param_name.clone(), i_id.1.to_raw());
            ids_str.push(format!("<record>${param_name}"));
        });
        let ids_str = ids_str.into_iter().collect::<Vec<String>>().join(",");

        let qry = format!("SELECT * FROM {};", ids_str);
        let query = self.db.query(qry);
        let query = bindings
            .into_iter()
            .fold(query, |query, n_val| query.bind(n_val));
        let mut res = query.await?;
        let res: Vec<TaskRequestParticipantion> = res.take(0)?;
        Ok(res)
    }

    pub async fn process_payments(
        &self,
        to_user: &Thing,
        participation_ids: Vec<Thing>,
    ) -> AppResult<()> {
        let participations = self.get_ids(&participation_ids).await?;

        /*let tasks: Vec<_> = participations
        .into_iter()
        .map(|p|(p.lock.clone(), to_user.clone()))
        .map(|lock_tx_touser| {
            tokio::spawn(async {
                if lock_tx_touser.0.is_some() {
                    let res = lock_tx_service.process_locked_payment(&lock_tx_touser.0.unwrap(), &lock_tx_touser.1).await;
                }
            })
        })
        .collect();*/
        // TODO execute in separate tokio tasks
        for participation in participations {
            if let Some(locked) = participation.lock {
                // not returning on error so successful payments are made
                let pay_locked = LockTransactionDbService {
                    db: self.db,
                    ctx: &self.ctx,
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

    pub async fn delete(&self, participation_id: Thing) -> CtxResult<bool> {
        let _res: Option<TaskRequestParticipantion> = self
            .db
            .delete((participation_id.tb, participation_id.id.to_raw()))
            .await?;
        Ok(true)
    }
}
