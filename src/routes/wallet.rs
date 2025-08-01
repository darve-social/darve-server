use std::collections::HashMap;
use std::sync::Arc;

use crate::entities::user_auth::local_user_entity;
use crate::entities::user_notification::UserNotificationEvent;
use crate::entities::wallet::gateway_transaction_entity::GatewayTransactionDbService;
use crate::entities::wallet::{balance_transaction_entity, wallet_entity};
use crate::interfaces::repositories::user_notifications::UserNotificationsInterface;
use crate::middleware;
use crate::middleware::auth_with_login_access::AuthWithLoginAccess;
use crate::middleware::error::{AppError, CtxResult};
use crate::middleware::mw_ctx::CtxState;
use crate::middleware::utils::db_utils::QryOrder::DESC;
use crate::middleware::utils::extractor_utils::JsonOrFormValidated;
use crate::middleware::utils::string_utils::get_string_thing;
use crate::utils::paypal::Paypal;
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use balance_transaction_entity::BalanceTransactionDbService;
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::utils::db_utils::{Pagination, ViewFieldSelector};
use middleware::utils::extractor_utils::DiscussionParams;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use stripe::{AccountId, Client, CreatePaymentIntent, Currency};
use surrealdb::sql::Thing;
use validator::Validate;
use wallet_entity::{CurrencySymbol, UserView, WalletDbService};

pub fn routes(is_development: bool) -> Router<Arc<CtxState>> {
    let mut router = Router::new()
        .route("/api/wallet/history", get(get_wallet_history))
        .route("/api/wallet/balance", get(get_user_balance))
        .route("/api/wallet/withdraw", post(withdraw))
        .route("/api/wallet/deposit", post(deposit));

    if is_development {
        router = router.route("/test/api/endow/:endow_user_id/:amount", get(test_deposit));
    }

    router
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
    fn get_select_query_fields() -> String {
        "id, wallet.{user.{id, username, full_name}, id }, with_wallet.{user.{id, username, full_name}, id }, balance, amount_in, amount_out, currency, r_created, r_updated".to_string()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WalletUserView {
    pub id: Thing,
    pub user: Option<UserView>,
}

pub async fn get_user_balance(
    auth_data: AuthWithLoginAccess,
    State(ctx_state): State<Arc<CtxState>>,
) -> CtxResult<Html<String>> {
    let user_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };
    let wallet_service = WalletDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };
    let user_id = user_service.get_ctx_user_thing().await?;
    let balances_view = wallet_service.get_user_balances(&user_id).await?;
    auth_data.ctx.to_htmx_or_json(balances_view)
}

pub async fn get_wallet_history(
    State(ctx_state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Query(params): Query<DiscussionParams>,
) -> CtxResult<Html<String>> {
    let user_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };
    let user_id = user_service.get_ctx_user_thing().await?;
    let pagination = Some(Pagination {
        order_by: Some("r_created".to_string()),
        order_dir: Some(DESC),
        count: params.count.unwrap_or(20),
        start: params.start.unwrap_or(0),
    });
    let tx_service = BalanceTransactionDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };
    let user_wallet_id = WalletDbService::get_user_wallet_id(&user_id);
    let transactions = tx_service
        .user_transaction_list(&user_wallet_id, pagination)
        .await?;
    auth_data
        .ctx
        .to_htmx_or_json(CurrencyTransactionHistoryView {
            wallet: user_wallet_id,
            transactions,
        })
}

#[derive(Debug, Deserialize, Validate)]
struct WithdrawData {
    #[validate(range(min = 100))]
    amount: u64,
}

async fn withdraw(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    JsonOrFormValidated(data): JsonOrFormValidated<WithdrawData>,
) -> CtxResult<()> {
    let user_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    };
    let user = user_service.get_ctx_user().await?;

    if user.email_verified.is_none() {
        return Err(AppError::Generic {
            description: "User email must be verified".to_string(),
        }
        .into());
    }

    let gateway_tx_service = GatewayTransactionDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    let gateway_tx_id = gateway_tx_service
        .user_withdraw_tx_start(
            &user.id.as_ref().unwrap(),
            data.amount as i64,
            "".to_string(),
        )
        .await?;

    let paypal = Paypal::new(
        &state.paypal_client_id,
        &state.paypal_client_key,
        &state.paypal_webhook_id,
    );

    let res = paypal
        .send_money(
            &gateway_tx_id.to_raw(),
            &user.email_verified.unwrap(),
            (data.amount as f64) / 100.00,
            &CurrencySymbol::USD.to_string(),
        )
        .await;

    match res {
        Ok(_) => Ok(()),
        Err(e) => {
            let _ = gateway_tx_service
                .user_withdraw_tx_revert(gateway_tx_id)
                .await;
            Err(AppError::Generic { description: e }.into())
        }
    }
}

#[derive(Debug, Validate, Deserialize)]
struct EndowmentData {
    #[validate(range(min = 100))]
    amount: u64,
}
async fn deposit(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    JsonOrFormValidated(data): JsonOrFormValidated<EndowmentData>,
) -> CtxResult<Json<String>> {
    let user_id = ctx.user_id()?;
    println!("User ID retrieved: {:?}", user_id);

    let acc_id = AccountId::from_str(&state.stripe_platform_account.as_str()).map_err(|e1| {
        ctx.to_ctx_error(AppError::Stripe {
            source: e1.to_string(),
        })
    })?;
    let client = Client::new(state.stripe_secret_key.clone()).with_stripe_account(acc_id.clone());

    let amt = data.amount as i64;

    let product_title = "wallet_endowment".to_string();
    let mut metadata = HashMap::with_capacity(3);
    metadata.insert("user_id".to_string(), user_id.clone());
    metadata.insert("amount".to_string(), amt.to_string());
    metadata.insert("action".to_string(), product_title.clone());

    let create_pi = CreatePaymentIntent {
        amount: amt,
        currency: Currency::USD,
        metadata: Some(metadata),
        on_behalf_of: None,
        transfer_data: None,
        application_fee_amount: None,
        automatic_payment_methods: None,
        capture_method: None,
        confirm: Some(false),
        customer: None,
        description: None,
        payment_method: None,
        receipt_email: None,
        return_url: None,
        setup_future_usage: None,
        shipping: None,
        statement_descriptor: None,
        statement_descriptor_suffix: None,
        transfer_group: None,
        use_stripe_sdk: None,
        mandate: None,
        mandate_data: None,
        off_session: None,
        payment_method_options: None,
        payment_method_types: None,
        confirmation_method: None,
        error_on_requires_action: None,
        expand: &[],
        payment_method_configuration: None,
        payment_method_data: None,
        radar_options: None,
    };

    let payment_intent = stripe::PaymentIntent::create(&client, create_pi)
        .await
        .map_err(|e| {
            ctx.to_ctx_error(AppError::Stripe {
                source: e.to_string(),
            })
        })?;

    Ok(Json(payment_intent.client_secret.unwrap()))
}

async fn test_deposit(
    State(ctx_state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path((endow_user_id, amount)): Path<(String, i64)>,
) -> CtxResult<Response> {
    if !ctx_state.is_development {
        return Err(AppError::AuthorizationFail {
            required: "Endpoint only available in development mode".to_string(),
        }
        .into());
    }

    let another_user_thing = get_string_thing(endow_user_id)?;

    let fund_service = GatewayTransactionDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };
    let wallet_service = WalletDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };

    fund_service
        .user_deposit_tx(
            &another_user_thing,
            "ext_acc123".to_string(),
            "ext_tx_id_123".to_string(),
            amount,
            CurrencySymbol::USD,
        )
        .await?;

    let user1_bal = wallet_service.get_user_balance(&another_user_thing).await?;

    let _ = ctx_state
        .db
        .user_notifications
        .create(
            &another_user_thing.to_raw(),
            "update balance",
            &UserNotificationEvent::UserBalanceUpdate.as_str(),
            &vec![another_user_thing.to_raw()],
            None,
        )
        .await?;

    Ok((StatusCode::OK, user1_bal.balance_usd.to_string()).into_response())
}
