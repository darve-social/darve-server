use crate::database::client::Db;
use crate::entities::community::post_entity;
use crate::entities::task_request_user::TaskRequestUserStatus;
use crate::entities::user_auth::local_user_entity;
use crate::entities::wallet::wallet_entity;
use crate::middleware;
use crate::middleware::utils::db_utils::get_entity_view;
use middleware::utils::db_utils::{
    get_entity, get_entity_list_view, with_not_found_err, IdentIdName, Pagination,
    ViewFieldSelector,
};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use surrealdb::sql::Thing;
use wallet_entity::CurrencySymbol;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub from_user: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_post: Option<Thing>,
    pub request_txt: String,
    pub deliverable_type: DeliverableType,
    pub r#type: TaskRequestType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deliverables: Option<Vec<Thing>>,
    pub reward_type: RewardType,
    pub currency: CurrencySymbol,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub enum TaskRequestType {
    Open,
    Close,
}

#[derive(Display, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RewardType {
    // needs to be same as col name
    // #[strum(to_string = "on_delivery")]
    OnDelivery, // paid when delivered
                // #[strum(to_string = "vote_winner")]
                // VoteWinner { voting_period_min: i64 }, // paid after voting
}

#[derive(EnumString, Display, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DeliverableType {
    PublicPost,
    // Participants,
}

pub struct TaskRequestDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

impl<'a> TaskRequestDbService<'a> {}

pub const TABLE_NAME: &str = "task_request";
const TABLE_COL_POST: &str = post_entity::TABLE_NAME;
const TABLE_COL_USER: &str = local_user_entity::TABLE_NAME;

impl<'a> TaskRequestDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let curr_usd = CurrencySymbol::USD.to_string();
        let curr_reef = CurrencySymbol::REEF.to_string();
        let curr_eth = CurrencySymbol::ETH.to_string();

        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS on_post ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_COL_POST}>>;
    DEFINE INDEX IF NOT EXISTS on_post_idx ON TABLE {TABLE_NAME} COLUMNS on_post;
    DEFINE FIELD IF NOT EXISTS from_user ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE INDEX IF NOT EXISTS from_user_idx ON TABLE {TABLE_NAME} COLUMNS from_user;
    DEFINE FIELD IF NOT EXISTS deliverable_type ON TABLE {TABLE_NAME} TYPE {{ type: \"PublicPost\"}};
    DEFINE FIELD IF NOT EXISTS request_txt ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD IF NOT EXISTS reward_type ON TABLE {TABLE_NAME} TYPE {{ type: \"OnDelivery\"}} | {{ type: \"VoteWinner\", voting_period_min: int }};
    DEFINE FIELD IF NOT EXISTS currency ON TABLE {TABLE_NAME} TYPE '{curr_usd}'|'{curr_reef}'|'{curr_eth}';
    DEFINE FIELD IF NOT EXISTS type ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.db.query(sql).await?;

        mutation.check().expect("should mutate taskRequest");

        Ok(())
    }

    pub async fn create(&self, record: TaskRequest) -> CtxResult<TaskRequest> {
        let res = self
            .db
            .create(TABLE_NAME)
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<TaskRequest>| v.unwrap());

        // let things: Vec<Domain> = self.db.select(TABLE_NAME).await.ok().unwrap();
        // dbg!(things);
        res
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<TaskRequest> {
        let opt = get_entity::<TaskRequest>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

    pub async fn get_by_id<T: for<'de> Deserialize<'de> + ViewFieldSelector>(
        &self,
        id: &Thing,
    ) -> CtxResult<T> {
        let opt = get_entity_view::<T>(
            &self.db,
            TABLE_NAME.to_string(),
            &IdentIdName::Id(id.clone()),
        )
        .await?;
        with_not_found_err(opt, self.ctx, &id.to_raw())
    }

    pub async fn get_by_user<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        user: &Thing,
        status: Option<TaskRequestUserStatus>,
    ) -> CtxResult<Vec<T>> {
        let status_query = match &status {
            Some(_) => "AND $status IN ->task_request_user.status",
            None => "",
        };
        let query = format!(
            "SELECT {} FROM {TABLE_NAME} WHERE $user IN ->task_request_user.out {};",
            &T::get_select_query_fields(&IdentIdName::Id(user.clone())),
            status_query
        );
        let mut res = self
            .db
            .query(query)
            .bind(("user", user.clone()))
            .bind(("status", status))
            .await?;
        Ok(res.take::<Vec<T>>(0)?)
    }

    pub async fn get_by_creator<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        user: Thing,
        post_id: Option<Thing>,
        pagination: Option<Pagination>,
    ) -> CtxResult<Vec<T>> {
        let mut filter_by = vec![IdentIdName::ColumnIdent {
            column: "from_user".to_string(),
            val: user.to_raw(),
            rec: true,
        }];
        if post_id.is_some() {
            filter_by.push(IdentIdName::ColumnIdent {
                column: "on_post".to_string(),
                val: post_id.expect("checked is some").to_raw(),
                rec: true,
            })
        }
        get_entity_list_view::<T>(
            self.db,
            TABLE_NAME.to_string(),
            &IdentIdName::ColumnIdentAnd(filter_by),
            pagination,
        )
        .await
    }

    pub async fn on_post_list_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        post_id: Thing,
    ) -> CtxResult<Vec<T>> {
        get_entity_list_view::<T>(
            self.db,
            TABLE_NAME.to_string(),
            &IdentIdName::ColumnIdent {
                column: "on_post".to_string(),
                val: post_id.to_raw(),
                rec: true,
            },
            None,
        )
        .await
    }

    // pub async fn add_participation(
    //     &self,
    //     offer_id: Thing,
    //     user_id: Thing,
    //     amount: i64,
    // ) -> CtxResult<TaskRequest> {
    //     // update existing item from user or push new one
    //     //TODO convert to db transactions
    //     let offer = self.get(IdentIdName::Id(offer_id.clone())).await?;
    //     let lock_service = LockTransactionDbService {
    //         db: self.db,
    //         ctx: self.ctx,
    //     };

    //     let mut participants = self
    //         .task_participation_repo
    //         .get_ids(&offer.participants)
    //         .await?;
    //     match participants.iter().position(|op| op.user == user_id) {
    //         None => {
    //             let lock = lock_service
    //                 .lock_user_asset_tx(
    //                     &user_id,
    //                     amount,
    //                     offer.currency.clone(),
    //                     vec![UnlockTrigger::Timestamp {
    //                         at: offer.valid_until,
    //                     }],
    //                 )
    //                 .await?;
    //             let partic_new = self
    //                 .task_participation_repo
    //                 .create_update(TaskRequestParticipation {
    //                     id: None,
    //                     amount,
    //                     lock: Some(lock),
    //                     user: user_id,
    //                     votes: None,
    //                     currency: offer.currency,
    //                 })
    //                 .await?;
    //             participants.push(partic_new);
    //         }
    //         Some(i) => {
    //             let partic = &mut participants[i];
    //             // unlock and lock different amt
    //             let existing_lock_id = partic.lock.clone();
    //             if let Some(lock) = existing_lock_id {
    //                 lock_service.unlock_user_asset_tx(&lock).await?;
    //             }

    //             let lock = lock_service
    //                 .lock_user_asset_tx(
    //                     &user_id,
    //                     amount,
    //                     offer.currency.clone(),
    //                     vec![UnlockTrigger::Timestamp {
    //                         at: offer.valid_until,
    //                     }],
    //                 )
    //                 .await?;
    //             partic.lock = Some(lock);
    //             partic.amount = amount;
    //             self.task_participation_repo
    //                 .create_update(partic.to_owned())
    //                 .await?;
    //         }
    //     }
    //     let partic_ids: Vec<Thing> = participants
    //         .iter()
    //         .map(|p| p.id.clone().expect("Saved participants with existing id"))
    //         .collect();
    //     let res: Option<TaskRequest> = self
    //         .db
    //         .update((offer_id.tb.clone(), offer_id.id.clone().to_string()))
    //         .patch(PatchOp::replace("/participants", partic_ids))
    //         .await
    //         .map_err(CtxError::from(self.ctx))?;
    //     res.ok_or_else(|| {
    //         self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
    //             ident: offer_id.to_raw(),
    //         })
    //     })
    // }

    // pub async fn remove_participation(
    //     &self,
    //     offer_id: Thing,
    //     user_id: Thing,
    // ) -> CtxResult<Option<TaskRequest>> {
    //     // if last user remove task request else update participants
    //     let offer = self.get(IdentIdName::Id(offer_id.clone())).await?;

    //     let mut participants = self
    //         .task_participation_repo
    //         .get_ids(&offer.participants)
    //         .await?;

    //     if let Some(i) = participants
    //         .iter()
    //         .position(|partic| partic.user == user_id)
    //     {
    //         let lock_service = LockTransactionDbService {
    //             db: self.db,
    //             ctx: self.ctx,
    //         };
    //         let existing_lock_id = participants[i].lock.clone();
    //         if let Some(lock) = existing_lock_id {
    //             lock_service.unlock_user_asset_tx(&lock).await?;
    //         }
    //         if participants.len() == 1 {
    //             self.delete(offer.id.expect("existing offer has id"))
    //                 .await?;
    //             return Ok(None);
    //         }
    //         participants.remove(i);
    //         let partic_ids: Vec<Thing> = participants
    //             .iter()
    //             .map(|p| p.id.clone().expect("Saved participants with existing id"))
    //             .collect();
    //         let res: Option<TaskRequest> = self
    //             .db
    //             .update((offer_id.tb.clone(), offer_id.id.clone().to_string()))
    //             .patch(PatchOp::replace("/participants", partic_ids))
    //             .await
    //             .map_err(CtxError::from(self.ctx))?;

    //         if res.is_some() {
    //             Ok(res)
    //         } else {
    //             Err(self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
    //                 ident: offer_id.to_raw(),
    //             }))
    //         }
    //     } else {
    //         Err(self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
    //             ident: "User participation not found".to_string(),
    //         }))
    //     }
    // }

    // not public - can only be deleted by removing participants
    // async fn delete(&self, offer_id: Thing) -> CtxResult<bool> {
    //     let lock_service = LockTransactionDbService {
    //         db: self.db,
    //         ctx: self.ctx,
    //     };

    //     let offer = self.get(IdentIdName::Id(offer_id.clone())).await?;
    //     if offer.participants.len() > 1 {
    //         return Err(self.ctx.to_ctx_error(AppError::Generic {
    //             description: "Can not delete with other participants".to_string(),
    //         }));
    //     }
    //     let participants = self
    //         .task_participation_repo
    //         .get_ids(&offer.participants)
    //         .await?;

    //     for partic in participants {
    //         let existing_lock_id = partic.lock.clone();
    //         if let Some(lock) = existing_lock_id {
    //             lock_service.unlock_user_asset_tx(&lock).await?;
    //         }
    //         self.task_participation_repo
    //             .delete(partic.id.unwrap())
    //             .await?;
    //     }
    //     let _: Option<TaskRequest> = self.db.delete((offer_id.tb, offer_id.id.to_raw())).await?;
    //     Ok(true)
    // }

    /*    pub async fn get_ids(&self, ids: Vec<Thing>) -> CtxResult<Vec<TaskRequest>> {
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
            let res: Vec<TaskRequest> = res.take(0)?;
            Ok(res)
        }
    */
}
