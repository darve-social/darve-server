use std::collections::HashMap;
use std::sync::Arc;

use crate::entities::user_auth::local_user_entity::{self};
use crate::entities::user_notification::UserNotificationEvent;
use crate::entities::wallet::balance_transaction_entity::TransactionType;
use crate::entities::wallet::gateway_transaction_entity::{
    GatewayTransaction, GatewayTransactionDbService, GatewayTransactionStatus,
};
use crate::entities::wallet::{balance_transaction_entity, wallet_entity};
use crate::interfaces::repositories::user_notifications::UserNotificationsInterface;
use crate::middleware;
use crate::middleware::auth_with_login_access::AuthWithLoginAccess;
use crate::middleware::error::{AppError, CtxResult};
use crate::middleware::mw_ctx::CtxState;
use crate::middleware::utils::db_utils::QryOrder::{self};
use crate::middleware::utils::extractor_utils::JsonOrFormValidated;
use crate::middleware::utils::string_utils::get_string_thing;
use crate::models::view::balance_tx::CurrencyTransactionView;
use crate::utils::paypal::Paypal;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use balance_transaction_entity::BalanceTransactionDbService;
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::utils::db_utils::Pagination;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use stripe::{AccountId, Client, CreatePaymentIntent, Currency};
use validator::Validate;
use wallet_entity::{CurrencySymbol, WalletDbService};

pub fn routes(is_development: bool) -> Router<Arc<CtxState>> {
    let mut router: Router<Arc<CtxState>> = Router::new()
        .route("/api/wallet/history", get(get_wallet_history))
        .route("/api/wallet/balance", get(get_user_balance))
        .route("/api/wallet/withdraw", post(withdraw))
        .route("/api/wallet/deposit", post(deposit))
        .route("/api/gateway_wallet/history", get(gateway_wallet_history));

    if is_development {
        router = router.route("/test/api/endow/:endow_user_id/:amount", get(test_deposit));
    }

    router
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

#[derive(Debug, Deserialize, Serialize)]
pub struct GetGatewayWalletHistoryQuery {
    pub order_by: Option<String>,
    pub order_dir: Option<QryOrder>,
    pub count: Option<u16>,
    pub start: Option<u32>,
    pub status: Option<GatewayTransactionStatus>,
    pub r#type: Option<TransactionType>,
}

pub async fn gateway_wallet_history(
    auth_data: AuthWithLoginAccess,
    State(ctx_state): State<Arc<CtxState>>,
    Query(params): Query<GetGatewayWalletHistoryQuery>,
) -> CtxResult<Json<Vec<GatewayTransaction>>> {
    let user_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };
    let user_id = user_service.get_ctx_user_thing().await?;
    let pagination = Some(Pagination {
        order_by: params.order_by.or(Some("created_at".to_string())),
        order_dir: params.order_dir.or(Some(QryOrder::DESC)),
        count: params.count.unwrap_or(20),
        start: params.start.unwrap_or(0),
    });
    let tx_service = GatewayTransactionDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };
    let transactions = tx_service
        .get_by_user(&user_id, params.status, params.r#type, pagination)
        .await?;

    Ok(Json(transactions))
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetWalletHistoryQuery {
    pub order_by: Option<String>,
    pub order_dir: Option<QryOrder>,
    pub count: Option<u16>,
    pub start: Option<u32>,
    pub r#type: Option<TransactionType>,
}

pub async fn get_wallet_history(
    auth_data: AuthWithLoginAccess,
    State(ctx_state): State<Arc<CtxState>>,
    Query(params): Query<GetWalletHistoryQuery>,
) -> CtxResult<Json<Vec<CurrencyTransactionView>>> {
    let user_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };
    let user_id = user_service.get_ctx_user_thing().await?;
    let pagination = Some(Pagination {
        order_by: params.order_by.or(Some("created_at".to_string())),
        order_dir: params.order_dir.or(Some(QryOrder::DESC)),
        count: params.count.unwrap_or(20),
        start: params.start.unwrap_or(0),
    });
    let tx_service = BalanceTransactionDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };
    let user_wallet_id = WalletDbService::get_user_wallet_id(&user_id);
    let transactions = tx_service
        .user_transaction_list(&user_wallet_id, params.r#type, pagination)
        .await?;

    Ok(Json(transactions))
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
        .user_withdraw_tx_start(&user.id.as_ref().unwrap(), data.amount as i64, None)
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
                .user_withdraw_tx_revert(gateway_tx_id, Some(e.clone()))
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
    user_auth: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
    JsonOrFormValidated(data): JsonOrFormValidated<EndowmentData>,
) -> CtxResult<Json<String>> {
    let user = LocalUserDbService {
        db: &state.db.client,
        ctx: &user_auth.ctx,
    }
    .get_by_id(&user_auth.user_thing_id())
    .await?;
    println!(
        "User ID retrieved: {:?}",
        user.id.as_ref().unwrap().to_raw()
    );

    let acc_id = AccountId::from_str(&state.stripe_platform_account.as_str()).map_err(|e1| {
        AppError::Stripe {
            source: e1.to_string(),
        }
    })?;
    let client = Client::new(state.stripe_secret_key.clone()).with_stripe_account(acc_id.clone());

    let amt = data.amount as i64;

    let product_title = "wallet_endowment".to_string();
    let id = GatewayTransactionDbService::generate_id();

    let mut metadata = HashMap::with_capacity(4);
    metadata.insert("tx_id".to_string(), id.to_raw());
    metadata.insert("user_id".to_string(), user.id.as_ref().unwrap().to_raw());
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
        .map_err(|e| AppError::Stripe {
            source: e.to_string(),
        })?;

    let _ = GatewayTransactionDbService {
        db: &state.db.client,
        ctx: &user_auth.ctx,
    }
    .user_deposit_start(
        id,
        user.id.as_ref().unwrap().clone(),
        amt,
        CurrencySymbol::USD,
        payment_intent.id.to_string(),
    )
    .await?;

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

    let tx = fund_service
        .user_deposit_start(
            GatewayTransactionDbService::generate_id(),
            another_user_thing.clone(),
            amount,
            CurrencySymbol::USD,
            "ext_tx_id_123".to_string(),
        )
        .await?;

    let wallet_service = WalletDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };

    let _res = fund_service
        .user_deposit_tx(
            tx,
            "ext_tx_id_123".to_string(),
            amount,
            CurrencySymbol::USD,
            None,
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
