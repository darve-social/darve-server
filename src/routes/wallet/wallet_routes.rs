use askama::Template;
use axum::extract::State;
use axum::response::sse::Event;
use axum::response::{Html, Sse};
use axum::routing::get;
use axum::Router;
use currency_transaction_entity::CurrencyTransactionDbService;
use futures::stream::Stream as FStream;
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::{IdentIdName, Pagination, ViewFieldSelector};
use middleware::utils::extractor_utils::DiscussionParams;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use user_auth::user_notification_routes::create_user_notifications_sse;
use user_notification_entity::{UserNotification, UserNotificationEvent};
use wallet_entity::{CurrencySymbol, UserView, WalletDbService};

use crate::entities::user_auth::{local_user_entity, user_notification_entity};
use crate::entities::wallet::{currency_transaction_entity, wallet_entity};
use crate::middleware;
use crate::routes::user_auth;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/user/wallet/history", get(get_wallet_history))
        .route("/api/user/wallet/balance", get(get_user_balance))
        .route("/api/user/wallet/balance/sse", get(get_balance_update_sse))
        .with_state(state)
}

#[derive(Template, Deserialize, Debug, Serialize)]
#[template(path = "nera2/default-content.html")]
pub struct CurrencyTransactionHistoryView {
    pub wallet: Thing,
    pub transactions: Vec<CurrencyTransactionView>,
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
        "id, wallet.{user.{id, username, full_name}, id }, with_wallet.{user.{id, username, full_name}, id }, balance, amount_in, amount_out, currency, r_created, r_updated".to_string()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WalletUserView {
    pub id: Thing,
    pub user: Option<UserView>,
}

pub async fn get_user_balance(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
) -> CtxResult<Html<String>> {
    let user_service = LocalUserDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };
    let wallet_service = WalletDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };
    let user_id = user_service.get_ctx_user_thing().await?;
    let balances_view = wallet_service.get_user_balances(&user_id).await?;
    ctx.to_htmx_or_json(balances_view)
}

pub async fn get_wallet_history(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    params: DiscussionParams,
) -> CtxResult<Html<String>> {
    let user_service = LocalUserDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };
    let user_id = user_service.get_ctx_user_thing().await?;
    let pagination = Some(Pagination {
        order_by: Some("id".to_string()),
        order_dir: None,
        count: params.count.unwrap_or(20),
        start: params.start.unwrap_or(0),
    });
    let tx_service = CurrencyTransactionDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };
    let user_wallet_id = WalletDbService::get_user_wallet_id(&user_id);
    let transactions = tx_service
        .user_transaction_list(&user_wallet_id, pagination)
        .await?;
    ctx.to_htmx_or_json(CurrencyTransactionHistoryView {
        wallet: user_wallet_id,
        transactions,
    })
}

async fn get_balance_update_sse(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
) -> CtxResult<Sse<impl FStream<Item = Result<Event, surrealdb::Error>>>> {
    create_user_notifications_sse(
        &_db,
        ctx,
        Vec::from([UserNotificationEvent::UserBalanceUpdate.to_string()]),
        to_sse_event,
    )
    .await?
}

fn to_sse_event(_ctx: Ctx, notification: UserNotification) -> CtxResult<Event> {
    let event_ident = notification.event.to_string();
    let event = match notification.event {
        UserNotificationEvent::UserBalanceUpdate { .. } => {
            Event::default().data("").event(event_ident)
        }
        _ => Event::default()
            .data(format!("Event ident {event_ident} recognised"))
            .event("Error".to_string()),
    };

    Ok(event)
}
