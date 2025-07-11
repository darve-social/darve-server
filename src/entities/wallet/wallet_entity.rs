use askama_axum::Template;

use crate::database::client::Db;
use middleware::utils::db_utils::{
    get_entity, get_entity_view, record_exists, with_not_found_err, IdentIdName, ViewFieldSelector,
};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use strum::Display;
use surrealdb::err::Error;
use surrealdb::sql::Thing;
use surrealdb::Response;

use super::balance_transaction_entity;
use crate::entities::wallet::balance_transaction_entity::{
    BalanceTransactionDbService, THROW_BALANCE_TOO_LOW,
};
use crate::middleware;
use crate::middleware::error::{AppResult, CtxError};

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
                surrealdb::Error::Api(surrealdb::error::Api::Query(msg))
                    if msg.contains(THROW_BALANCE_TOO_LOW) =>
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
const TRANSACTION_TABLE: &str = balance_transaction_entity::TABLE_NAME;
pub const TRANSACTION_HEAD_F: &str = "transaction_head";

impl<'a> WalletDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let curr_usd = CurrencySymbol::USD;
        let curr_reef = CurrencySymbol::REEF;
        let curr_eth = CurrencySymbol::ETH;
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS {TRANSACTION_HEAD_F} ON TABLE {TABLE_NAME} TYPE object;
    DEFINE FIELD IF NOT EXISTS {TRANSACTION_HEAD_F}.{curr_usd} ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>>;
    DEFINE FIELD IF NOT EXISTS {TRANSACTION_HEAD_F}.{curr_reef} ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>>;
    DEFINE FIELD IF NOT EXISTS {TRANSACTION_HEAD_F}.{curr_eth} ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>>;
    DEFINE FIELD IF NOT EXISTS lock_id ON TABLE {TABLE_NAME} TYPE option<string> ASSERT {{
    IF $before==NONE || $value==NONE || $before<time::now() {{
        RETURN true 
    }} ELSE {{
        THROW \"{THROW_WALLET_LOCKED}\"
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
