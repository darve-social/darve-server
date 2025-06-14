use askama_axum::Template;
use balance_transaction_entity::BalanceTransactionDbService;

use crate::database::client::Db;
use middleware::utils::db_utils::{
    get_entity, get_entity_view, record_exists, with_not_found_err, IdentIdName, ViewFieldSelector,
};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use strum::Display;
use surrealdb::err::Error;
use surrealdb::sql::Thing;
use surrealdb::Response;

use super::balance_transaction_entity;
use crate::entities::user_auth::local_user_entity;
use crate::entities::wallet::balance_transaction_entity::THROW_BALANCE_TOO_LOW;
use crate::middleware;
use crate::middleware::error::AppResult;

pub fn check_transaction_custom_error(query_response: &mut Response) -> AppResult<()> {
    let query_err = query_response
        .take_errors()
        .values()
        .fold(None, |ret, error| {
            if let Some(AppError::WalletLocked) = ret {
                return ret;
            }
            if let Some(AppError::BalanceTooLow) = ret {
                return ret;
            }

            match error {
                surrealdb::Error::Db(Error::Thrown(throw_val))
                    if throw_val == THROW_WALLET_LOCKED =>
                {
                    Some(AppError::WalletLocked)
                }
                surrealdb::Error::Db(Error::Thrown(throw_val))
                    if throw_val == THROW_BALANCE_TOO_LOW =>
                {
                    Some(AppError::BalanceTooLow)
                }
                surrealdb::Error::Db(Error::QueryNotExecuted) if ret.is_some() => ret,
                _ => Some(AppError::SurrealDb {
                    source: error.to_string(),
                }),
            }
        });
    match query_err {
        None => Ok(()),
        Some(err) => Err(err),
    }
}

pub(crate) static APP_GATEWAY_WALLET: Lazy<Thing> =
    Lazy::new(|| Thing::from((TABLE_NAME, "app_gateway_wallet")));
pub const THROW_WALLET_LOCKED: &str = "Wallet locked";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wallet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub user: Option<Thing>,
    pub transaction_head: WalletCurrencyTxHeads,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalletCurrencyTxHeads {
    usd: Option<Thing>,
    eth: Option<Thing>,
    reef: Option<Thing>,
}

#[derive(Display, Clone, Serialize, Deserialize, Debug)]
pub enum CurrencySymbol {
    USD,
    REEF,
    ETH,
}

// TODO -fixed_decimals- save as fixed value in db and use display_decimals for UI
#[allow(dead_code)]
impl CurrencySymbol {
    fn fixed_decimals(&self) -> u32 {
        match self {
            CurrencySymbol::USD => 2,
            CurrencySymbol::REEF => 18,
            CurrencySymbol::ETH => 18,
        }
    }

    fn display_decimal(&self, _balance_fixed: i64, _display_number_decimals: u8) -> i64 {
        todo!(); // TODO return display value with specific number of decimals for UI
    }
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/default-content.html")]
pub struct WalletBalancesView {
    pub id: Thing,
    pub balance: WalletBalanceView,
    pub balance_locked: WalletBalanceView,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WalletBalanceView {
    pub id: Thing,
    pub balance_usd: i64,
    pub balance_reef: i64,
    pub balance_eth: i64,
}

impl ViewFieldSelector for WalletBalanceView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        let curr_usd = CurrencySymbol::USD.to_string();
        let curr_reef = CurrencySymbol::REEF.to_string();
        let curr_eth = CurrencySymbol::ETH.to_string();
        format!("id, user.{{id, username, full_name}}, {TRANSACTION_HEAD_F}.{curr_usd}.*.balance||0 as balance_usd, {TRANSACTION_HEAD_F}.{curr_reef}.*.balance||0 as balance_reef, {TRANSACTION_HEAD_F}.{curr_eth}.*.balance||0 as balance_eth")
    }
}

#[derive(Template, Serialize, Deserialize, Debug, Clone)]
#[template(path = "nera2/default-content.html")]
pub struct UserView {
    pub id: Thing,
    pub username: String,
    pub full_name: Option<String>,
}

pub struct WalletDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "wallet";
const USER_TABLE: &str = local_user_entity::TABLE_NAME;
const TRANSACTION_TABLE: &str = balance_transaction_entity::TABLE_NAME;

pub const TRANSACTION_HEAD_F: &str = "transaction_head";

impl<'a> WalletDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let curr_usd = CurrencySymbol::USD;
        let curr_reef = CurrencySymbol::REEF;
        let curr_eth = CurrencySymbol::ETH;
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS user ON TABLE {TABLE_NAME} TYPE option<record<{USER_TABLE}>> VALUE $before OR $value; //TODO type::record({USER_TABLE}:record::id($this.id));
    DEFINE INDEX IF NOT EXISTS user_idx ON TABLE {TABLE_NAME} COLUMNS user;
    DEFINE FIELD IF NOT EXISTS {TRANSACTION_HEAD_F} ON TABLE {TABLE_NAME} TYPE object;
    DEFINE FIELD IF NOT EXISTS {TRANSACTION_HEAD_F}.{curr_usd} ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>>;
    DEFINE FIELD IF NOT EXISTS {TRANSACTION_HEAD_F}.{curr_reef} ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>>;
    DEFINE FIELD IF NOT EXISTS {TRANSACTION_HEAD_F}.{curr_eth} ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>>;
    DEFINE FIELD IF NOT EXISTS lock_id ON TABLE {TABLE_NAME} TYPE option<string> ASSERT {{
    IF $before==NONE || $value==NONE || $before==$value {{
        RETURN true
    }} ELSE {{
        THROW \"{THROW_WALLET_LOCKED}\"//+<string>($before)+\" vv=\"+<string>($value)
    }} }};
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    // DEFINE INDEX IF NOT EXISTS r_created_idx ON TABLE {TABLE_NAME} COLUMNS r_created;
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();

    ");
        let mutation = self.db.query(sql).await?;

        mutation.check().expect("should mutate wallet");

        Ok(())
    }

    pub async fn get_user_balances(&self, user_id: &Thing) -> CtxResult<WalletBalancesView> {
        // TODO merge to single query
        let balance = self.get_user_balance(user_id).await?;
        let balance_locked = self.get_user_balance_locked(user_id).await?;
        Ok(WalletBalancesView {
            id: user_id.clone(),
            balance,
            balance_locked,
        })
    }

    pub async fn get_user_balance(&self, user_id: &Thing) -> CtxResult<WalletBalanceView> {
        let user_wallet_id = &Self::get_user_wallet_id(user_id);
        self.get_balance(user_wallet_id).await
    }

    pub async fn get_user_balance_locked(&self, user_id: &Thing) -> CtxResult<WalletBalanceView> {
        let user_wallet_id = &Self::get_user_lock_wallet_id(user_id);
        self.get_balance(user_wallet_id).await
    }

    pub async fn get_balance(&self, wallet_id: &Thing) -> CtxResult<WalletBalanceView> {
        Self::is_wallet_id(self.ctx.clone(), wallet_id)?;
        if record_exists(self.db, wallet_id).await.is_ok() {
            self.get_view::<WalletBalanceView>(IdentIdName::Id(wallet_id.clone()))
                .await
        } else {
            Ok(WalletBalanceView {
                id: wallet_id.clone(),
                balance_usd: 0,
                balance_reef: 0,
                balance_eth: 0,
            })
        }
    }

    pub fn is_wallet_id(ctx: Ctx, wallet_id: &Thing) -> CtxResult<()> {
        if wallet_id.tb != TABLE_NAME {
            return Err(ctx.to_ctx_error(AppError::Generic {
                description: "wrong tb in wallet_id".to_string(),
            }));
        }
        Ok(())
    }

    pub(crate) async fn init_app_gateway_wallet(&self) -> CtxResult<WalletBalanceView> {
        let wallet_id: &Thing = &APP_GATEWAY_WALLET.clone();
        Self::is_wallet_id(self.ctx.clone(), wallet_id)?;
        if record_exists(self.db, &wallet_id).await.is_ok() {
            return Err(self.ctx.to_ctx_error(AppError::Generic {
                description: "Wallet already exists".to_string(),
            }));
        }
        let currency_symbol = CurrencySymbol::USD;
        let init_tx_usd = BalanceTransactionDbService {
            db: self.db,
            ctx: self.ctx,
        }
        .create_init_record(&wallet_id, currency_symbol.clone())
        .await?;
        let currency_symbol = CurrencySymbol::REEF;
        let init_tx_reef = BalanceTransactionDbService {
            db: self.db,
            ctx: self.ctx,
        }
        .create_init_record(&wallet_id, currency_symbol.clone())
        .await?;
        let currency_symbol = CurrencySymbol::ETH;
        let init_tx_eth = BalanceTransactionDbService {
            db: self.db,
            ctx: self.ctx,
        }
        .create_init_record(&wallet_id, currency_symbol.clone())
        .await?;

        // let gtw_wallet = APP_GATEWAY_WALLET.clone();
        // let user = if wallet_id==&gtw_wallet {
        //     None
        // }else{ Some(Self::get_user_id(wallet_id))};

        let wallet = self
            .db
            .create(TABLE_NAME)
            .content(Wallet {
                id: Some(wallet_id.clone()),
                user: None,
                transaction_head: WalletCurrencyTxHeads {
                    usd: Some(init_tx_usd.id.unwrap()),
                    eth: Some(init_tx_eth.id.unwrap()),
                    reef: Some(init_tx_reef.id.unwrap()),
                },
                r_created: None,
                r_updated: None,
            })
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<Wallet>| v.unwrap())?;
        Ok(WalletBalanceView {
            id: wallet.id.unwrap(),
            balance_usd: init_tx_usd.balance,
            balance_reef: init_tx_reef.balance,
            balance_eth: init_tx_eth.balance,
        })
    }

    pub(crate) fn get_user_wallet_id(user_id: &Thing) -> Thing {
        // Thing::from((TABLE_NAME, format!("{}_u", ident.id).as_str()))
        Thing::from((TABLE_NAME, user_id.id.clone()))
    }

    pub(crate) fn get_user_lock_wallet_id(user_id: &Thing) -> Thing {
        // Thing::from((TABLE_NAME, format!("{}_u", ident.id).as_str()))
        Thing::from((
            TABLE_NAME,
            format!("{}_{}", user_id.id.clone(), "locked").as_str(),
        ))
    }

    // not used anywhere - commenting for now @anukulpandey
    // pub(crate) fn get_user_id(wallet_id: &Thing) -> Thing {
    //     Thing::from((USER_TABLE, wallet_id.id.clone()))
    // }

    pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        ident_id_name: IdentIdName,
    ) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<Wallet> {
        let opt = get_entity::<Wallet>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }
}

// TODO: -wallet tests- move to /tests and fix

// #[cfg(test)]
// mod tests {
//     use crate::entities::user_auth;
//     use crate::entities::wallet::wallet_entity::{
//         CurrencySymbol, WalletDbService, APP_GATEWAY_WALLET,
//     };
//     use crate::entities::wallet::{
//         balance_transaction_entity, gateway_transaction_entity, lock_transaction_entity,
//     };
//     use crate::middleware;
//     use chrono::{Duration, Utc};
//     use balance_transaction_entity::{CurrencyTransaction, CurrencyTransactionDbService};
//     use gateway_transaction_entity::GatewayTransactionDbService;
//     use lock_transaction_entity::{LockTransaction, LockTransactionDbService, UnlockTrigger};
//     use middleware::ctx::Ctx;
//
//     use middleware::error::AppResult;
//     use middleware::utils::db_utils::IdentIdName;
//     use middleware::utils::string_utils::get_string_thing;
//     use serde::{Deserialize, Serialize};
//     use strum::Display;
//     use surrealdb::engine::any::connect;
//     use surrealdb::sql::Thing;
//     use surrealdb::{Surreal, Uuid};
//     use tokio::io::AsyncWriteExt;
//     use tokio_stream::StreamExt;
//     use user_auth::authentication_entity::AuthType;
//     use user_auth::local_user_entity::{LocalUser, LocalUserDbService};

//     #[tokio::test]
//     async fn endow_lock_wallet() {
//         let (db, ctx) = init_db_test().await;

//         let user_db_service = LocalUserDbService { db: &db, ctx: &ctx };
//         let usr1 = user_db_service
//             .create(
//                 LocalUser {
//                     id: None,
//                     username: "usname1".to_string(),
//                     full_name: None,
//                     birth_date: None,
//                     phone: None,
//                     email: None,
//                     bio: None,
//                     social_links: None,
//                     image_uri: None,
//                 },
//                 AuthType::PASSWORD(Some("pass123".to_string())),
//             )
//             .await
//             .expect("user id");

//         let fund_service = GatewayTransactionDbService { db: &db, ctx: &ctx };
//         let lock_service = LockTransactionDbService { db: &db, ctx: &ctx };
//         let wallet_service = WalletDbService { db: &db, ctx: &ctx };
//         let tx_service = CurrencyTransactionDbService { db: &db, ctx: &ctx };

//         let user1 = get_string_thing(usr1).expect("got thing");
//         let endow_tx_id = fund_service
//             .user_endowment_tx(
//                 &user1,
//                 "ext_acc123".to_string(),
//                 "ext_tx_id_123".to_string(),
//                 100,
//                 CurrencySymbol::USD,
//             )
//             .await
//             .expect("created");

//         let user1_bal = wallet_service
//             .get_user_balance(&user1)
//             .await
//             .expect("got balance");
//         assert_eq!(user1_bal.balance_usd, 100);
//         let gtw_bal = wallet_service
//             .get_balance(&APP_GATEWAY_WALLET.clone())
//             .await
//             .expect("got balance");
//         assert_eq!(gtw_bal.balance_usd, -100);

//         let user1_wallet = wallet_service
//             .get(IdentIdName::Id(user1_bal.id))
//             .await
//             .expect("wallet");
//         let user_tx = tx_service
//             .get(IdentIdName::Id(user1_wallet.transaction_head.usd.unwrap()))
//             .await
//             .expect("user");

//         assert_eq!(user_tx.gateway_tx.expect("ident"), endow_tx_id);
//         assert_eq!(user_tx.with_wallet, APP_GATEWAY_WALLET.clone());
//         // dbg!(&user_tx);

//         let lock_amount = 33;
//         let lock_tx = lock_service
//             .lock_user_asset_tx(
//                 &user1,
//                 lock_amount,
//                 CurrencySymbol::USD,
//                 vec![UnlockTrigger::Timestamp {
//                     at: Utc::now().checked_add_signed(Duration::days(5)).unwrap(),
//                 }],
//             )
//             .await
//             .expect("locked");

//         let mut lock_tx = lock_service
//             .db
//             .query(format!("SELECT * FROM {lock_tx} "))
//             .await
//             .unwrap();
//         let lck: Option<LockTransaction> = lock_tx.take(0).unwrap();
//         let lck = lck.unwrap();

//         assert_eq!(lck.unlock_triggers.len(), 1);

//         let mut lock_transfer_tx = tx_service
//             .db
//             .query(format!(
//                 "SELECT * FROM {} ",
//                 lck.lock_tx_out.unwrap().to_raw()
//             ))
//             .await
//             .unwrap();
//         let c_tx: Option<CurrencyTransaction> = lock_transfer_tx.take(0).unwrap();
//         let curr_tx = c_tx.unwrap();

//         assert_eq!(curr_tx.amount_out.unwrap(), lock_amount);
//         assert_eq!(curr_tx.balance, 67);

//         let lock_w_id = WalletDbService::get_user_lock_wallet_id(&user1);
//         let lock_wallet = wallet_service.get_balance(&lock_w_id).await.unwrap();
//         let user_wallet = wallet_service.get_user_balance(&user1).await.unwrap();

//         assert_eq!(lock_wallet.balance_usd, lock_amount);
//         assert_eq!(user_wallet.balance_usd, 100 - lock_amount);

//         let lock_tx = lock_service
//             .lock_user_asset_tx(
//                 &user1,
//                 33333,
//                 CurrencySymbol::REEF,
//                 vec![UnlockTrigger::Timestamp {
//                     at: Utc::now().checked_add_signed(Duration::days(5)).unwrap(),
//                 }],
//             )
//             .await;
//         assert_eq!(lock_tx.is_err(), true);

//         let unlck = lock_service
//             .unlock_user_asset_tx(&lck.id.unwrap())
//             .await
//             .unwrap();

//         let lock_wallet = wallet_service.get_balance(&lock_w_id).await.unwrap();
//         let user_wallet = wallet_service.get_user_balance(&user1).await.unwrap();

//         assert_eq!(lock_wallet.balance_usd, 0);
//         assert_eq!(user_wallet.balance_usd, 100);
//     }

//     #[tokio::test]
//     async fn query_with_params() {
//         let (db, ctx) = init_db_test().await;

//         let user_db_service = LocalUserDbService { db: &db, ctx: &ctx };
//         let usr1 = user_db_service
//             .create(
//                 LocalUser {
//                     id: None,
//                     username: "usname1".to_string(),
//                     full_name: None,
//                     birth_date: None,
//                     phone: None,
//                     email: None,
//                     bio: None,
//                     social_links: None,
//                     image_uri: None,
//                 },
//                 AuthType::PASSWORD(Some("pass123".to_string())),
//             )
//             .await
//             .expect("user id");

//         // let usr1 = LocalUserDbService{ db: &db, ctx: &ctx }.get(IdentIdName::Id(get_string_thing(usr1).unwrap())).await.expect("got user");
//         let usr1 = user_db_service
//             .get(IdentIdName::ColumnIdent {
//                 column: "id".to_string(),
//                 val: get_string_thing(usr1).unwrap().to_raw(),
//                 rec: true,
//             })
//             .await
//             .expect("got user");
//         dbg!(usr1);
//     }

//     #[tokio::test]
//     async fn prod_balance_0() {
//         let (db, ctx) = init_db_test().await;

//         let usr1 = LocalUserDbService { db: &db, ctx: &ctx }
//             .create(
//                 LocalUser {
//                     id: None,
//                     username: "usname1".to_string(),
//                     full_name: None,
//                     birth_date: None,
//                     phone: None,
//                     email: None,
//                     bio: None,
//                     social_links: None,
//                     image_uri: None,
//                 },
//                 AuthType::PASSWORD(Some("pass123".to_string())),
//             )
//             .await
//             .expect("user");

//         let user_thing = get_string_thing(usr1.clone()).expect("thing1");
//         let balance_view1 = WalletDbService { db: &db, ctx: &ctx }
//             .get_user_balance(&user_thing)
//             .await
//             .expect("balance");
//         // dbg!(&balance_view1);
//         assert_eq!(
//             balance_view1.id,
//             WalletDbService::get_user_wallet_id(&user_thing)
//         );
//         assert_eq!(balance_view1.balance_usd, 0);
//     }

//     #[tokio::test]
//     async fn make_balance_tx() {
//         let (db, ctx) = init_db_test().await;

//         let usr1 = LocalUserDbService { db: &db, ctx: &ctx }
//             .create(
//                 LocalUser {
//                     id: None,
//                     username: "usname1".to_string(),
//                     full_name: None,
//                     birth_date: None,
//                     phone: None,
//                     email: None,
//                     bio: None,
//                     social_links: None,
//                     image_uri: None,
//                 },
//                 AuthType::PASSWORD(Some("pass123".to_string())),
//             )
//             .await
//             .expect("user");

//         let usr2 = LocalUserDbService { db: &db, ctx: &ctx }
//             .create(
//                 LocalUser {
//                     id: None,
//                     username: "usname2".to_string(),
//                     full_name: None,
//                     birth_date: None,
//                     phone: None,
//                     email: None,
//                     bio: None,
//                     social_links: None,
//                     image_uri: None,
//                 },
//                 AuthType::PASSWORD(Some("pass234".to_string())),
//             )
//             .await
//             .expect("user2");

//         let wallet_service = WalletDbService { db: &db, ctx: &ctx };
//         let transaction_db_service = CurrencyTransactionDbService { db: &db, ctx: &ctx };

//         let user1_thing = get_string_thing(usr1.clone()).expect("thing1");

//         // endow usr1
//         let balance_view1 = wallet_service
//             .get_user_balance(&user1_thing)
//             .await
//             .expect("balance");
//         // dbg!(&balance_view1);
//         assert_eq!(
//             balance_view1.id.clone(),
//             WalletDbService::get_user_wallet_id(&user1_thing)
//         );
//         assert_eq!(balance_view1.balance_usd, 0);

//         let endowment_service = GatewayTransactionDbService { db: &db, ctx: &ctx };
//         let _endow_usr1 = endowment_service
//             .user_endowment_tx(
//                 &get_string_thing(usr1.clone()).unwrap(),
//                 "ext_acc333".to_string(),
//                 "endow_tx_usr1".to_string(),
//                 100,
//                 CurrencySymbol::USD,
//             )
//             .await
//             .expect("is ok");
//         let _endow_usr2 = endowment_service
//             .user_endowment_tx(
//                 &get_string_thing(usr2.clone()).unwrap(),
//                 "ext_acc333".to_string(),
//                 "endow_tx_usr2".to_string(),
//                 100,
//                 CurrencySymbol::USD,
//             )
//             .await
//             .expect("is ok");
//         let _endow_usr2r = endowment_service
//             .user_endowment_tx(
//                 &get_string_thing(usr2.clone()).unwrap(),
//                 "ext_acc333".to_string(),
//                 "endow_tx_usr2-reef".to_string(),
//                 10000,
//                 CurrencySymbol::REEF,
//             )
//             .await
//             .expect("is ok");

//         let gtw_bal = wallet_service
//             .get_balance(&APP_GATEWAY_WALLET.clone())
//             .await
//             .expect("got balance");
//         assert_eq!(gtw_bal.balance_usd, -200);

//         let balance_view1 = wallet_service
//             .get_user_balance(&user1_thing)
//             .await
//             .expect("balance");
//         // dbg!(&balance_view1);
//         assert_eq!(
//             balance_view1.id.clone(),
//             WalletDbService::get_user_wallet_id(&user1_thing)
//         );
//         assert_eq!(balance_view1.balance_usd, 100);

//         let user2_thing = get_string_thing(usr2.clone()).expect("thing2");
//         let balance_view2 = wallet_service
//             .get_user_balance(&user2_thing)
//             .await
//             .expect("balance");
//         // dbg!(&balance_view2)
//         assert_eq!(
//             balance_view2.id.clone(),
//             WalletDbService::get_user_wallet_id(&user2_thing)
//         );
//         assert_eq!(balance_view2.balance_usd, 100);

//         let balance_view1_before_tx = wallet_service
//             .get_user_balance(&user1_thing)
//             .await
//             .expect("balance");
//         dbg!(&balance_view1_before_tx);

//         let moved = transaction_db_service
//             .transfer_currency(
//                 &balance_view2.id,
//                 &balance_view1.id,
//                 432,
//                 &CurrencySymbol::REEF,
//             )
//             .await;

//         let moved = transaction_db_service
//             .transfer_currency(
//                 &balance_view1.id,
//                 &balance_view2.id,
//                 77,
//                 &CurrencySymbol::USD,
//             )
//             .await;

//         let balance_view1 = wallet_service
//             .get_user_balance(&user1_thing)
//             .await
//             .expect("balance");
//         dbg!(&balance_view1);
//         assert_eq!(
//             balance_view1.id.clone(),
//             WalletDbService::get_user_wallet_id(&user1_thing)
//         );
//         assert_eq!(balance_view1.balance_usd, 23);

//         let balance_view2 = wallet_service
//             .get_user_balance(&user2_thing)
//             .await
//             .expect("balance");
//         dbg!(&balance_view2);
//         assert_eq!(
//             balance_view2.id.clone(),
//             WalletDbService::get_user_wallet_id(&user2_thing)
//         );
//         assert_eq!(balance_view2.balance_usd, 177);

//         let moved = transaction_db_service
//             .transfer_currency(
//                 &balance_view1.id,
//                 &balance_view2.id,
//                 24,
//                 &CurrencySymbol::USD,
//             )
//             .await; //.expect("move balance");
//         assert_eq!(moved.is_err(), true);
//         let moved = transaction_db_service
//             .transfer_currency(
//                 &balance_view1.id,
//                 &balance_view2.id,
//                 23,
//                 &CurrencySymbol::USD,
//             )
//             .await; //.expect("move balance");
//         assert_eq!(moved.is_err(), false);

//         let moved = transaction_db_service
//             .transfer_currency(
//                 &balance_view1.id,
//                 &balance_view2.id,
//                 23,
//                 &CurrencySymbol::ETH,
//             )
//             .await;
//         assert_eq!(moved.is_err(), true);

//         let txs = transaction_db_service
//             .user_transaction_list(&WalletDbService::get_user_wallet_id(&user1_thing), None)
//             .await
//             .expect("result");
//         assert_eq!(txs.len(), 4);
//         let tx_0 = txs.get(0).expect("tx0");
//         assert_eq!(tx_0.balance, 100);
//         assert_eq!(tx_0.amount_in.expect("has amt"), 100);
//         assert_eq!(tx_0.with_wallet.user.is_none(), true);

//         let tx_1 = txs.get(1).expect("tx1");
//         assert_eq!(tx_1.balance, 432);
//         assert_eq!(tx_1.amount_in.expect("has amt"), 432);
//         assert_eq!(tx_1.currency.to_string(), CurrencySymbol::REEF.to_string());
//         assert_eq!(tx_1.with_wallet.user.is_none(), false);

//         let tx_2 = txs.get(2).expect("tx2");
//         assert_eq!(tx_2.balance, 23);
//         assert_eq!(tx_2.amount_out.expect("has amt"), 77);
//         assert_eq!(tx_2.with_wallet.user.is_none(), false);

//         let tx_3 = txs.get(3).expect("tx3");
//         assert_eq!(tx_3.balance, 0);
//         assert_eq!(tx_3.amount_out.expect("has amt"), 23);
//         assert_eq!(tx_3.with_wallet.user.is_none(), false);

//         let gateway_wallet = wallet_service
//             .get_balance(&APP_GATEWAY_WALLET.clone())
//             .await
//             .expect("wallet");
//         dbg!(gateway_wallet);
//     }

//     // derive Display only stringifies enum ident, serde also serializes the value
//     #[derive(Debug, PartialEq, Serialize, Deserialize, Display)]
//     pub enum SomeTestEnum {
//         UserFollowAdded {
//             username: String,
//             rec: Thing,
//             opt: Option<String>,
//         },
//         UserTaskRequestComplete {
//             task_id: String,
//             deliverables: Vec<String>,
//         },
//     }

//     #[derive(Serialize, Deserialize, Debug)]
//     struct Val {
//         id: Option<Thing>,
//         value: SomeTestEnum,
//     }

//     #[tokio::test]
//     async fn test_enum_field_literal() {
//         let (db, ctx) = init_db_test().await;
//         let qry = r#"DEFINE TABLE IF NOT EXISTS test_enum SCHEMAFULL;
//     DEFINE FIELD IF NOT EXISTS value ON TABLE test_enum TYPE {UserFollowAdded:{username:string, rec: record, opt: option<string>}} | {UserTaskRequestComplete:{task_id: string, deliverables:array<string>}};"#;

//         &db.query(qry).await.expect("table defined");

//         let s = serde_json::to_string(&SomeTestEnum::UserFollowAdded {
//             username: "usss".to_string(),
//             rec: Thing::from(("test_enum", "32432fa")),
//             opt: Some("vall".to_string()),
//         })
//         .expect("string");
//         println!("hhh={}", s);

//         let uuu: SomeTestEnum = serde_json::from_str(s.as_str()).expect("back");
//         dbg!(&uuu);

//         println!("{}", &uuu.to_string());
//         // derive Display only stringifies enum ident, serde also serializes the value
//         assert_eq!("UserFollowAdded", &uuu.to_string());

//         let res: Option<Val> = db
//             .create("test_enum")
//             .content(Val {
//                 id: None,
//                 value: uuu,
//             })
//             .await
//             .expect("saved");
//         dbg!(&res);
//         let res: Option<Val> = db
//             .select(("test_enum", res.unwrap().id.unwrap().id.to_raw()))
//             .await
//             .expect("rec");
//         dbg!(res);

//         let res: Option<Val> = db
//             .create("test_enum")
//             .content(Val {
//                 id: None,
//                 value: SomeTestEnum::UserTaskRequestComplete {
//                     task_id: "taaask:123".to_string(),
//                     deliverables: vec!["one".to_string()],
//                 },
//             })
//             .await
//             .expect("saved");
//         dbg!(&res);
//         let res: Option<Val> = db
//             .select(("test_enum", res.unwrap().id.unwrap().id.to_raw()))
//             .await
//             .expect("rec");
//         dbg!(res);
//     }

//     async fn backup(_db: Db) {
//         let mut backup = _db.export(()).await.unwrap();
//         let mut file = tokio::fs::OpenOptions::new()
//             .write(true)
//             .create(true)
//             .open("/Users/mac02/dev/DB_BACKUP.surql")
//             .await
//             .unwrap();
//         // println!("DB BBACC={:?}", file.metadata().unwrap());
//         while let Some(result) = backup.next().await {
//             match result {
//                 Ok(bytes) => {
//                     file.write_all(bytes.as_slice()).await.unwrap();
//                 }
//                 Err(error) => {
//                     // Handle the export error
//                     println!("ERRRRRR {}", error);
//                 }
//             }
//         }
//     }

//     async fn run_migrations(db: Db) -> AppResult<()> {
//         let c = Ctx::new(Ok("migrations".parse().unwrap()), Uuid::new_v4(), false);

//         LocalUserDbService { db: &db, ctx: &c }.mutate_db().await?;
//         WalletDbService { db: &db, ctx: &c }.mutate_db().await?;
//         CurrencyTransactionDbService { db: &db, ctx: &c }
//             .mutate_db()
//             .await?;
//         LockTransactionDbService { db: &db, ctx: &c }
//             .mutate_db()
//             .await?;

//         Ok(())
//     }

//     async fn init_db_test() -> (Db, Ctx) {
//         let db = connect("mem://").await.unwrap();
//         db.use_ns("namespace").use_db("database").await.unwrap();
//         run_migrations(db.clone()).await.expect("migrations run");
//         let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);
//         (db, ctx)
//     }
// }
