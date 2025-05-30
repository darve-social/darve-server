use chrono::{DateTime, Utc};
use currency_transaction_entity::CurrencyTransactionDbService;
use middleware::db;
use middleware::utils::db_utils::{get_entity, with_not_found_err, IdentIdName};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
use wallet_entity::{CurrencySymbol, WalletDbService};

use crate::entities::user_auth::local_user_entity;
use crate::middleware;

use super::{currency_transaction_entity, wallet_entity};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LockTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub lock_tx_out: Option<Thing>,
    pub unlock_tx_in: Option<Thing>,
    pub unlock_triggers: Vec<UnlockTrigger>,
    pub user: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
// #[serde(tag = "type")]  - this has issues with array desirializing if there's type: inside
pub enum UnlockTrigger {
    // UserRequest{user_id: Thing},
    // Delivery{post_id: Thing},
    Timestamp { at: DateTime<Utc> },
}

pub struct LockTransactionDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "lock_transaction";
const USER_TABLE: &str = local_user_entity::TABLE_NAME;
const TRANSACTION_TABLE: &str = currency_transaction_entity::TABLE_NAME;

impl<'a> LockTransactionDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS lock_tx_out ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>> VALUE $before OR $value;
    DEFINE FIELD IF NOT EXISTS unlock_tx_in ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>> VALUE $before OR $value;
    DEFINE FIELD IF NOT EXISTS user ON TABLE {TABLE_NAME} TYPE record<{USER_TABLE}>;
    DEFINE FIELD IF NOT EXISTS unlock_triggers ON TABLE {TABLE_NAME} TYPE array<{{\"UserRequest\":{{ \"user_id\":record}} }}|{{\"Delivery\":{{ \"post_id\":record}} }}|{{\"Timestamp\":{{ \"at\":string}} }}>;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();

    ");
        let mutation = self.db.query(sql).await?;

        mutation.check().expect("should mutate lockTransaction");

        Ok(())
    }

    pub async fn lock_user_asset_tx(
        &self,
        user: &Thing,
        amount: i64,
        currency_symbol: CurrencySymbol,
        unlock_triggers: Vec<UnlockTrigger>,
    ) -> CtxResult<Thing> {
        let user_wallet = WalletDbService::get_user_wallet_id(user);

        let lock_tx_id = Thing::from((TABLE_NAME, Id::ulid()));

        let user_lock_wallet = WalletDbService::get_user_lock_wallet_id(user);
        let user_2_lock_tx = CurrencyTransactionDbService::get_transfer_qry(
            &user_wallet,
            &user_lock_wallet,
            amount,
            &currency_symbol,
            None,
            Some(lock_tx_id.clone()),
            true,
        )?;
        let user_2_lock_qry = user_2_lock_tx.get_query_string();

        let fund_qry = format!(
            "
        BEGIN TRANSACTION;

           {user_2_lock_qry}

            LET $lock_tx = INSERT INTO {TABLE_NAME} {{
                id: $l_tx_id,
                user: $user,
                lock_tx_out: $tx_out_id,
                unlock_triggers: $un_triggers,
            }} RETURN id;

            RETURN $lock_tx[0].id;
        COMMIT TRANSACTION;

        "
        );
        let qry = self
            .db
            .query(fund_qry)
            .bind(("l_tx_id", lock_tx_id))
            .bind(("lock_amt", amount))
            .bind(("user", user.clone()))
            .bind(("un_triggers", unlock_triggers))
            .bind(("currency", currency_symbol));

        let qry = user_2_lock_tx
            .get_bindings()
            .iter()
            .fold(qry, |q, item| q.bind((item.0.clone(), item.1.clone())));

        let mut lock_res = qry.await?;
        lock_res = lock_res.check()?;
        let res: Option<Thing> = lock_res.take(0)?;
        res.ok_or(self.ctx.to_ctx_error(AppError::Generic {
            description: "Error in lock tx".to_string(),
        }))
    }

    pub async fn unlock_user_asset_tx(&self, lock_id: &Thing) -> CtxResult<LockTransaction> {
        // TODO do checks in db transaction
        let lock = self.get(IdentIdName::Id(lock_id.clone())).await?;
        let (amount, currency_symbol) = if lock.unlock_tx_in.is_some() {
            return Err(self.ctx.to_ctx_error(AppError::Generic {
                description: "Already unlocked".to_string(),
            }));
        } else if lock.lock_tx_out.is_none() {
            return Err(self.ctx.to_ctx_error(AppError::Generic {
                description: "Lock out tx does not exist".to_string(),
            }));
        } else {
            let lock_tx = CurrencyTransactionDbService {
                db: self.db,
                ctx: self.ctx,
            }
            .get(IdentIdName::Id(lock.lock_tx_out.expect("checked before")))
            .await?;
            (
                lock_tx
                    .amount_out
                    .ok_or(self.ctx.to_ctx_error(AppError::Generic {
                        description: "Lock out tx does have amount_out value".to_string(),
                    }))?,
                lock_tx.currency,
            )
        };

        let lock_tx_id = lock.id;
        let user_wallet = WalletDbService::get_user_wallet_id(&lock.user);
        let user_lock_wallet = WalletDbService::get_user_lock_wallet_id(&lock.user);
        let lock_2_user_tx = CurrencyTransactionDbService::get_transfer_qry(
            &user_lock_wallet,
            &user_wallet,
            amount,
            &currency_symbol,
            None,
            lock_tx_id.clone(),
            true,
        )?;
        let lock_2_user_qry = lock_2_user_tx.get_query_string();

        let fund_qry = format!(
            "
        BEGIN TRANSACTION;

           {lock_2_user_qry}

            LET $lock_tx = UPDATE $l_tx_id SET unlock_tx_in = $tx_in_id;

            RETURN $lock_tx[0];
        COMMIT TRANSACTION;

        "
        );
        let qry = self.db.query(fund_qry).bind(("l_tx_id", lock_tx_id));

        let qry = lock_2_user_tx
            .get_bindings()
            .iter()
            .fold(qry, |q, item| q.bind((item.0.clone(), item.1.clone())));

        let mut lock_res = qry.await?;
        lock_res = lock_res.check()?;
        let res: Option<LockTransaction> = lock_res.take(0)?;
        res.ok_or(self.ctx.to_ctx_error(AppError::Generic {
            description: "Error in unlock tx".to_string(),
        }))
    }

    pub async fn process_locked_payment(
        &self,
        lock_id: &Thing,
        pay_to_user: &Thing,
    ) -> CtxResult<()> {
        let curr_tx_service = CurrencyTransactionDbService {
            db: self.db,
            ctx: self.ctx,
        };

        // TODO !!! transfers in db transaction
        let unlocked = self.unlock_user_asset_tx(lock_id).await?;
        // dbg!(&unlocked);
        let unlock_tx_in =
            unlocked
                .unlock_tx_in
                .ok_or(self.ctx.to_ctx_error(AppError::Generic {
                    description: "No unlock tx in id".to_string(),
                }))?;
        let unlocked_tx_in = curr_tx_service.get(IdentIdName::Id(unlock_tx_in)).await?;
        let unlocked_amount =
            unlocked_tx_in
                .amount_in
                .ok_or(self.ctx.to_ctx_error(AppError::Generic {
                    description: "No unlocked in amount in unlock tx".to_string(),
                }))?;

        let wallet_to = WalletDbService::get_user_wallet_id(pay_to_user);
        let wallet_from = WalletDbService::get_user_wallet_id(&unlocked.user);
        curr_tx_service
            .transfer_currency(
                &wallet_from,
                &wallet_to,
                unlocked_amount,
                &unlocked_tx_in.currency,
            )
            .await?;
        Ok(())
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<LockTransaction> {
        let opt = get_entity::<LockTransaction>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }
}
