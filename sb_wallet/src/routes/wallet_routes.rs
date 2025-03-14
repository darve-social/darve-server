use std::str::FromStr;

use crate::entity::currency_transaction_entitiy::CurrencyTransactionDbService;
use crate::entity::wallet_entitiy::{CurrencySymbol, UserView};
use axum::extract::{Path, State};
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use sb_middleware::ctx::Ctx;
use sb_middleware::error::CtxResult;
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::{IdentIdName, ViewFieldSelector};
use sb_middleware::utils::string_utils::get_string_thing;
use serde::Deserialize;
use surrealdb::sql::Thing;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/user/:user_id/wallet_history", get(get_wallet_history))
        .with_state(state)
}

#[derive(Deserialize, Debug)]
pub struct CurrencyTransactionView {
    pub id: Thing,
    pub wallet: WalletUserView,
    pub with_wallet: WalletUserView,
    pub balance: i64,
    pub currency_symbol: CurrencySymbol,
    pub amount_in: Option<i64>,
    pub amount_out: Option<i64>,
    pub r_created: String,
    pub r_updated: String,
}

impl ViewFieldSelector for CurrencyTransactionView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, wallet.{user.{id, username, full_name} }, with_wallet.{user.{id, username, full_name} }, balance, amount_in, amount_out, currency_symbol, r_created, r_updated".to_string()
    }
}

#[derive(Deserialize, Debug)]
pub struct WalletUserView {
    pub user: UserView,
}

async fn get_wallet_history(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(user_id): Path<String>,
) -> CtxResult<Html<String>> {
    // let user_id = get_string_thing(user_id.clone())?;
   
    Ok("ok".to_string().into())
}