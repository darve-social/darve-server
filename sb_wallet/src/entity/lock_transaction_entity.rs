use std::collections::BTreeMap;
use std::time::Duration;
use crate::entity::currency_transaction_entitiy::CurrencyTransactionDbService;
use sb_middleware::db;
use sb_middleware::utils::db_utils::{
    get_entity, with_not_found_err, IdentIdName,
};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Object, Thing, Value};
use crate::entity::wallet_entitiy::{CurrencySymbol, WalletDbService, APP_GATEWAY_WALLET};

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
// #[serde(tag = "type")]
pub enum UnlockTrigger {π
    UserRequest{user_id: Thing},
    Delivery{post_id: Thing},
}

pub struct LockTransactionDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "lock_transaction";
const USER_TABLE: &str = sb_user_auth::entity::local_user_entity::TABLE_NAME;
const TRANSACTION_TABLE: &str = crate::entity::currency_transaction_entitiy::TABLE_NAME;

impl<'a> LockTransactionDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {

        // let curr_usd = CurrencySymbol::USD;
        // let curr_reef = CurrencySymbol::REEF;
        // let curr_eth = CurrencySymbol::ETH;
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD lock_tx_out ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>> VALUE $before OR $value;
    DEFINE FIELD unlock_tx_in ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>> VALUE $before OR $value;
    DEFINE FIELD user ON TABLE {TABLE_NAME} TYPE record<{USER_TABLE}>;
    DEFINE FIELD unlock_triggers ON TABLE {TABLE_NAME} TYPE array<{{\"UserRequest\":{{ \"user_id\":record}} }}|{{\"Delivery\":{{ \"post_id\":record}} }}>;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();

    ");
        let mutation = self.db.query(sql).await?;

        mutation.check().expect("should mutate lockTransaction");

        Ok(())
    }

    pub(crate) async fn lock_user_asset_tx(&self, user: &Thing, amount: i64, currency_symbol: CurrencySymbol, unlock_triggers:Vec<UnlockTrigger>) -> CtxResult<Thing> {

        let user_wallet = WalletDbService::get_user_wallet_id(user);
        
        // let lock_id = BTreeMap::from([("task_id".to_string(), Value::from("post_123")), ("user_id".to_string(), Value::from(user.to_raw().as_str())), ("timestamp".to_string(), Value::from("234"))]);
        let lock_tx_id = Thing::from((TABLE_NAME, Id::ulid()));

        let user_lock_wallet = WalletDbService::get_user_lock_wallet_id(user);
        let user_2_lock_tx = CurrencyTransactionDbService::get_transfer_qry(&user_wallet, &user_lock_wallet, amount, &currency_symbol,None, Some(lock_tx_id.clone()), true)?;
        let user_2_lock_qry = user_2_lock_tx.get_query_string();

        let fund_qry = format!("
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

        ");
        let qry = self.db.query(fund_qry)
            .bind(("l_tx_id", lock_tx_id))
            .bind(("lock_amt", amount))
            .bind(("user", user.clone()))
            .bind(("un_triggers", unlock_triggers))
            .bind(("currency", currency_symbol));
        
        let qry = user_2_lock_tx.get_bindings().iter().fold(qry, |q, item|{
            q.bind((item.0.clone(), item.1.clone()))
        });

        let mut lock_res = qry.await?;
        dbg!(&lock_res);
        lock_res = lock_res.check()?;
        let res:Option<Thing> = lock_res.take(0)?;
        res.ok_or(self.ctx.to_ctx_error(AppError::Generic {description:"Error in endowment tx".to_string()}))
    }


    pub(crate) async fn unlock_user_asset_tx(&self) -> CtxResult<()> {
        todo!()
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<LockTransaction> {
        let opt = get_entity::<LockTransaction>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }
}

