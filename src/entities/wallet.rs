use askama_axum::Template;
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use strum::Display;
use surrealdb::sql::{Id, Thing};
use surrealdb::Response;
use surrealdb::err::Error;

use crate::middleware::utils::db_utils::ViewFieldSelector;
use crate::utils::validate_utils::{deserialize_thing_or_string_id, serialize_string_id};
use crate::middleware::error::{AppError, AppResult, CtxResult};
use crate::middleware::ctx::Ctx;

pub mod balance_transaction_entity;
pub mod gateway_transaction_entity;

pub const TABLE_NAME: &str = "wallet";
pub const TRANSACTION_HEAD_F: &str = "transaction_head";
pub const THROW_WALLET_LOCKED: &str = "Wallet locked";
pub const THROW_BALANCE_TOO_LOW: &str = "Not enough balance"; 

pub fn is_wallet_id(ctx: &Ctx, wallet_id: &Thing) -> CtxResult<()> {
    if wallet_id.tb != TABLE_NAME {
        return Err(ctx.to_ctx_error(AppError::Generic {
            description: "wrong tb in wallet_id".to_string(),
        }));
    }
    Ok(())
}


pub static APP_GATEWAY_WALLET: Lazy<Thing> =
    Lazy::new(|| Thing::from((TABLE_NAME, "app_gateway_wallet")));
pub static DARVE_WALLET: Lazy<Thing> = Lazy::new(|| Thing::from((TABLE_NAME, "darve_wallet")));

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalletEntity {
    #[serde(deserialize_with = "deserialize_thing_or_string_id")]
    #[serde(serialize_with = "serialize_string_id")]
    pub id: String,
    
    pub transaction_head: WalletCurrencyTxHeads,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<DateTime<Utc>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalletCurrencyTxHeads {
    pub usd: Option<Thing>,
    pub eth: Option<Thing>,
    pub reef: Option<Thing>,
}

#[derive(Display, Clone, Serialize, Deserialize, Debug)]
pub enum CurrencySymbol {
    USD,
    REEF,
    ETH,
}

#[allow(dead_code)]
impl CurrencySymbol {
    pub fn fixed_decimals(&self) -> u32 {
        match self {
            CurrencySymbol::USD => 2,
            CurrencySymbol::REEF => 18,
            CurrencySymbol::ETH => 18,
        }
    }

    pub fn display_decimal(&self, _balance_fixed: i64, _display_number_decimals: u8) -> i64 {
        todo!(); 
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
    fn get_select_query_fields() -> String {
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

pub fn get_user_wallet_id(user_id: &Thing) -> Thing {
    Thing::from((TABLE_NAME, user_id.id.clone()))
}

pub fn get_user_lock_wallet_id(user_id: &Thing) -> Thing {
    Thing::from((
        TABLE_NAME,
        format!("{}_{}", user_id.id, "locked").as_str(),
    ))
}

pub fn generate_id() -> Thing {
    Thing::from((TABLE_NAME, Id::rand()))
}

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