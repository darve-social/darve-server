use std::str::FromStr;
use askama::Template;
use crate::entity::currency_transaction_entitiy::CurrencyTransactionDbService;
use crate::entity::wallet_entitiy::{CurrencySymbol, UserView, WalletDbService};
use axum::extract::{Path, State};
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use sb_middleware::ctx::Ctx;
use sb_middleware::error::CtxResult;
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::{get_entity_list_view, IdentIdName, Pagination, QryOrder, ViewFieldSelector};
use sb_middleware::utils::string_utils::get_string_thing;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use sb_middleware::utils::extractor_utils::DiscussionParams;
use sb_user_auth::entity::local_user_entity::LocalUserDbService;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/user/:user_id/wallet_history", get(get_wallet_history))
        .with_state(state)
}

#[derive(Template, Deserialize, Debug, Serialize)]
#[template(path = "nera2/default-content.html")]
pub struct CurrencyTransactionHistoryView {
    wallet: Thing,
    transactions: Vec<CurrencyTransactionView>,
}
#[derive(Template, Deserialize, Debug, Serialize)]
#[template(path = "nera2/default-content.html")]
pub struct CurrencyTransactionView {
    pub id: Thing,
    pub wallet: WalletUserView,
    pub with_wallet: WalletUserView,
    pub balance: i64,
    pub currency: CurrencySymbol,
    pub amount_in: Option<i64>,
    pub amount_out: Option<i64>,
    pub r_created: String,
    pub r_updated: String,
}

impl ViewFieldSelector for CurrencyTransactionView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, wallet.{user.{id, username, full_name} }, with_wallet.{user.{id, username, full_name} }, balance, amount_in, amount_out, currency, r_created, r_updated".to_string()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WalletUserView {
    pub user: Option<UserView>,
}

pub async fn get_wallet_history(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    params: DiscussionParams,
) -> CtxResult<Html<String>> {
    let user_service = LocalUserDbService { db: &ctx_state._db, ctx: &ctx };
    let user_id = user_service.get_ctx_user_thing().await?;
    let pagination = Some(Pagination {
        order_by: None,
        order_dir: None,
        count: params.count.unwrap_or(20),
        start: params.start.unwrap_or(0),
    });
    let tx_service = CurrencyTransactionDbService{ db: &ctx_state._db, ctx: &ctx };
    let user_wallet_id = WalletDbService::get_user_wallet_id(&user_id);
    let transactions = tx_service.user_transaction_list(&user_wallet_id, pagination).await?;
    ctx.to_htmx_or_json(CurrencyTransactionHistoryView{
        wallet: user_wallet_id,
        transactions,
    })
}