use crate::entity::task_request_entitiy::TaskRequestDbService;
use sb_middleware::db;
use sb_middleware::utils::db_utils::{get_entity, with_not_found_err, IdentIdName};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use strum::Display;
use surrealdb::opt::PatchOp;
use surrealdb::opt::Resource::RecordId;
use surrealdb::sql::{Id, Thing};
use tower::ServiceExt;
use sb_wallet::entity::lock_transaction_entity::{LockTransactionDbService, UnlockTrigger};
use sb_wallet::entity::wallet_entitiy::CurrencySymbol;

#[derive(Display, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RewardType {
    // needs to be same as col name
    // #[strum(to_string = "on_delivery")]
    OnDelivery, // paid when delivered
    // #[strum(to_string = "vote_winner")]
    VoteWinner { voting_period_min: i64 }, // paid after voting
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RewardVote {
    deliverable_ident: String,
    points: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParticipantReward {
    pub(crate) amount: i64,
    pub(crate) currency: CurrencySymbol,
    pub(crate) user: Thing,
    pub(crate) lock: Option<Thing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) votes: Option<Vec<RewardVote>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequestOffer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub task_request: Thing,
    pub user: Thing,
    pub reward_type: RewardType,
    pub participants: Vec<ParticipantReward>,
    pub valid_until: DateTime<Utc>,
    pub currency: CurrencySymbol,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

pub struct TaskRequestOfferDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "task_request_offer";
const TABLE_COL_USER: &str = sb_user_auth::entity::local_user_entity::TABLE_NAME;
const TABLE_COL_TASK_REQUEST: &str = crate::entity::task_request_entitiy::TABLE_NAME;
const LOCK_TABLE_NAME: &str = sb_wallet::entity::lock_transaction_entity::TABLE_NAME;

impl<'a> TaskRequestOfferDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let curr_usd = CurrencySymbol::USD.to_string();
        let curr_reef = CurrencySymbol::REEF.to_string();
        let curr_eth = CurrencySymbol::ETH.to_string();
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD user ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE INDEX user_idx ON TABLE {TABLE_NAME} COLUMNS user;
    DEFINE FIELD task_request ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_TASK_REQUEST}>;
    DEFINE INDEX user_treq_idx ON TABLE {TABLE_NAME} COLUMNS user, task_request UNIQUE;
    DEFINE FIELD reward_type ON TABLE {TABLE_NAME} TYPE {{ type: \"OnDelivery\"}} | {{ type: \"VoteWinner\", voting_period_min: int }};
    DEFINE FIELD participants ON TABLE {TABLE_NAME} TYPE array<{{ amount: int, user: record<{TABLE_COL_USER}>, votes: option<array<{{deliverable_ident: string, points: int, currency:'{curr_usd}'|'{curr_reef}'|'{curr_eth}', lock: record<{LOCK_TABLE_NAME}>}}>> }}>;
    DEFINE FIELD valid_until ON TABLE {TABLE_NAME} TYPE datetime;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.db.query(sql).await?;

        &mutation.check().expect("should mutate taskRequestOffer");

        Ok(())
    }

   /* pub async fn create_task_offer(
        &self,
        task_request: Thing,
        user: Thing,
        amount: i64,
    ) -> CtxResult<TaskRequestOffer> {
        // get task offer or create

        let offer = self
            .create_update(TaskRequestOffer {
                id: None,
                task_request: task_request.clone(),
                user: user.clone(),
                reward_type: RewardType::OnDelivery,
                participants: vec![RewardParticipant {
                    amount,
                    user: user,
                    votes: None,
                }],
                r_created: None,
                r_updated: None,
            })
            .await?;

        // if existing_offer_id.is_none() {
        TaskRequestDbService {
            db: self.db,
            ctx: self.ctx,
        }
            .add_offer(task_request, offer.id.clone().unwrap())
            .await?;
        // }

        Ok(offer)
    }*/

    pub async fn create_update(&self, mut record: TaskRequestOffer) -> CtxResult<TaskRequestOffer> {
        let resource = record
            .id
            .clone()
            .unwrap_or(Thing::from((TABLE_NAME.to_string(), Id::rand())));
        record.r_created = None;
        record.r_updated = None;

        self.db
            .upsert((resource.tb, resource.id.to_raw()))
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<TaskRequestOffer>| v.unwrap())
    }


    pub async fn add_participation(&self, offer_id: Thing, user_id: Thing, amount: i64) -> CtxResult<TaskRequestOffer> {
        // update existing item from user or push new one
        let mut offer = self.get(IdentIdName::Id(offer_id.clone())).await?;
        let lock_service = LockTransactionDbService { db: self.db, ctx: self.ctx };

        match offer.participants.iter().position(|rp| rp.user == user_id) {
            None => {
                let lock = lock_service.lock_user_asset_tx(&user_id, amount, offer.currency.clone(), vec![UnlockTrigger::Timestamp {at: offer.valid_until}]).await?;
                offer.participants.push(ParticipantReward {
                    amount,
                    lock: Some(lock),
                    user: user_id,
                    votes: None,
                    currency: offer.currency,
                })
            }
            Some(i) => {
                let partic = &mut offer.participants[i];
                // unlock and lock different amt
                let existing_lock_id = partic.lock.clone();
                if let Some(lock) = existing_lock_id{
                    lock_service.unlock_user_asset_tx(&lock).await?;
                }
                let lock = lock_service.lock_user_asset_tx(&user_id, amount, offer.currency.clone(), vec![UnlockTrigger::Timestamp {at: offer.valid_until}]).await?;
                partic.lock = Some(lock);
                partic.amount = amount;
            }
        }

        let res: Option<TaskRequestOffer> = self
            .db
            .update((offer_id.tb.clone(), offer_id.id.clone().to_string()))
            .patch(PatchOp::replace("/participants", offer.participants))
            .await
            .map_err(CtxError::from(self.ctx))?;
        res.ok_or_else(|| {
            self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
                ident: offer_id.to_raw(),
            })
        })
    }

    pub async fn remove_participation(&self, offer_id: Thing, user_id: Thing) -> CtxResult<Option<TaskRequestOffer>> {
        // if last user remove task request else update participants
        let mut offer = self.get(IdentIdName::Id(offer_id.clone())).await?;

        if let Some(i) = offer.participants.iter().position(|partic| partic.user == user_id) {
            let lock_service = LockTransactionDbService { db: self.db, ctx: self.ctx };
            let existing_lock_id = offer.participants[i].lock.clone();
            if let Some(lock) = existing_lock_id{
                lock_service.unlock_user_asset_tx(&lock).await?;
            }
            if offer.participants.len() < 2 {
                self.delete(offer.id.expect("existing offer has id"));
                return Ok(None);
            }
            offer.participants.remove(i);
            let res: Option<TaskRequestOffer> = self
                .db
                .update((offer_id.tb.clone(), offer_id.id.clone().to_string()))
                .patch(PatchOp::replace("/participants", offer.participants))
                .await
                .map_err(CtxError::from(self.ctx))?;

            if res.is_some() { Ok(res) } else {
                Err(self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
                    ident: offer_id.to_raw(),
                }))
            }
        } else { Err(self.ctx.to_ctx_error(AppError::EntityFailIdNotFound { ident: "User participation not found".to_string() })) }
    }

    // not public - can only be deleted by removing participants
    async fn delete(&self, offer_id: Thing) -> CtxResult<bool> {
        let lock_service = LockTransactionDbService { db: self.db, ctx: self.ctx };
        let mut offer = self.get(IdentIdName::Id(offer_id.clone())).await?;
        for partic in offer.participants {
            let existing_lock_id = partic.lock.clone();
            if let Some(lock) = existing_lock_id {
                lock_service.unlock_user_asset_tx(&lock).await?;
            }
        }
        let res: Option<TaskRequestOffer> = self.db.delete((offer_id.tb, offer_id.id.to_raw())).await?;
        Ok(true)
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<TaskRequestOffer> {
        let opt = get_entity::<TaskRequestOffer>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

    pub async fn get_ids(&self, ids: Vec<Thing>) -> CtxResult<Vec<TaskRequestOffer>> {
        let mut bindings: HashMap<String, String> = HashMap::new();
        let mut ids_str: Vec<String> = vec![];
        ids.into_iter().enumerate().for_each(|i_id| {
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
        let res: Vec<TaskRequestOffer> = res.take(0)?;
        Ok(res)
    }
}
