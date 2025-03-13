use std::io::Write;
use std::str::FromStr;

use askama_axum::axum_core::response::IntoResponse;
use askama_axum::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use axum::routing::{delete, get, post};
use axum::Router;
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use crate::entity::follow_entitiy::FollowDbService;
use crate::entity::local_user_entity::{LocalUser, LocalUserDbService};
use crate::entity::user_notification_entitiy::{
    UserNotification, UserNotificationDbService, UserNotificationEvent,
};
use sb_middleware::ctx::Ctx;
use sb_middleware::error::CtxResult;
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::{IdentIdName, ViewFieldSelector};
use sb_middleware::utils::request_utils::CreatedResponse;
use sb_middleware::utils::string_utils::get_string_thing;
use crate::entity::wallet_entitiy::{CurrencySymbol, WalletBalanceView};

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/user/:user_id/wallet_history", get(get_wallet_history))
        .with_state(state)
}

#[derive(Deserialize, Debug)]
pub struct CurrencyTransactionView {
    pub id: Thing,
    pub wallet: WalletBalanceView,
    pub with_wallet: WalletBalanceView,
    pub balance: i64,
    pub currency_symbol: CurrencySymbol,
    pub amount_in: Option<i64>,
    pub amount_out: Option<i64>,
    pub r_created: String,
    pub r_updated: String,
}

impl ViewFieldSelector for CurrencyTransactionView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        format!("id, wallet.{{}}, currency_symbol")
    }
}

async fn get_wallet_history(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(user_id): Path<String>,
) -> CtxResult<Html<String>> {
    let user_id = get_string_thing(user_id.clone())?;
    let followers: Vec<> = FollowDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .user_followers(user_id)
    .await?
    .into_iter()
    .map(UserItemView::from)
    .collect();
    ctx.to_htmx_or_json(UserListView { items: followers })
}
}
