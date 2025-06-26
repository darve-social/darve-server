use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::sync::Arc;

use askama_axum::axum_core::response::IntoResponse;
use axum::body::Body;
use axum::extract::{FromRef, FromRequest, Path, Request, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{async_trait, Router};
use gateway_transaction_entity::GatewayTransactionDbService;
use serde::Serialize;
use stripe::{AccountId, Client, CreatePaymentIntent, Currency, Event};
use surrealdb::sql::Thing;
use wallet_entity::{CurrencySymbol, WalletDbService};

use crate::entities::user_notification::UserNotificationEvent;
use crate::entities::wallet::{gateway_transaction_entity, wallet_entity};
use crate::interfaces::repositories::user_notifications::UserNotificationsInterface;
use crate::middleware;
use crate::middleware::mw_ctx::CtxState;
use crate::middleware::utils::extractor_utils::extract_stripe_event;
use middleware::ctx::Ctx;
use middleware::error::{AppError, CtxResult};
use middleware::utils::string_utils::get_string_thing;

const PRODUCT_ID_KEY: &str = "product_id";

pub fn routes(is_development: bool) -> Router<Arc<CtxState>> {
    let mut router = Router::new()
        .route(
            "/api/user/wallet/endowment/:amount",
            get(request_endowment_intent),
        )
        .route("/api/stripe/endowment/webhook", post(handle_webhook));

    if is_development {
        router = router.route(
            "/test/api/endow/:endow_user_id/:amount",
            get(test_endowment_transaction),
        );
    }

    router
}

struct EndowmentIdent {
    user_id: Thing,
    amount: i64,
    pub action: String,
}

impl Display for EndowmentIdent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}~{}~{}",
            self.user_id.to_raw().replace(":", "-"),
            self.amount,
            self.action
        )
    }
}

impl TryFrom<&String> for EndowmentIdent {
    type Error = AppError;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        let mut spl = value.as_str().split("~");
        let user_ident = spl.next().ok_or(AppError::Generic {
            description: "missing user_id part".to_string(),
        })?;
        let amount = spl.next().ok_or(AppError::Generic {
            description: "missing amount part".to_string(),
        })?;
        let amount = amount.parse::<i64>().map_err(|e| AppError::Generic {
            description: e.to_string(),
        })?;
        let action = spl
            .next()
            .ok_or(AppError::Generic {
                description: "missing action part".to_string(),
            })?
            .to_string();
        let user_id = get_string_thing(user_ident.replace("-", ":"))?;
        Ok(EndowmentIdent {
            user_id,
            amount,
            action,
        })
    }
}

async fn test_endowment_transaction(
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

async fn request_endowment_intent(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(amount): Path<u32>,
) -> CtxResult<Response> {
    let user_id = ctx.user_id()?;
    println!("User ID retrieved: {:?}", user_id);

    let acc_id = AccountId::from_str(&state.stripe_platform_account.as_str()).map_err(|e1| {
        ctx.to_ctx_error(AppError::Stripe {
            source: e1.to_string(),
        })
    })?;
    let client = Client::new(state.stripe_secret_key.clone()).with_stripe_account(acc_id.clone());

    let amt = (amount * 100) as i64;

    let product_title = "wallet_endowment".to_string();
    let endowment_id = EndowmentIdent {
        user_id: get_string_thing(user_id.clone())?,
        amount: amt,
        action: product_title.clone(),
    };

    let create_pi = CreatePaymentIntent {
        amount: amt,
        currency: Currency::USD,
        metadata: Some(std::collections::HashMap::from([(
            String::from(PRODUCT_ID_KEY),
            endowment_id.to_string(),
        )])),
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

    Ok((StatusCode::OK, payment_intent.client_secret.unwrap()).into_response())
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
struct StripeEvent(Event);

// TODO merge duplicate code in stripe_utils
#[async_trait]
impl<S> FromRequest<S> for StripeEvent
where
    String: FromRequest<S>,
    S: Send + Sync,
    CtxState: FromRef<S>,
{
    type Rejection = Response;

    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        let signature = if let Some(sig) = req.headers().get("stripe-signature") {
            sig.to_owned()
        } else {
            return Err(StatusCode::BAD_REQUEST.into_response());
        };

        let payload = String::from_request(req, state)
            .await
            .map_err(IntoResponse::into_response)?;

        let state = CtxState::from_ref(state);
        let wh_secret = state.stripe_wh_secret;

        let event =
            stripe::Webhook::construct_event(&payload, signature.to_str().unwrap(), &wh_secret);

        match event {
            Ok(e) => Ok(Self(e)),
            Err(e) => {
                println!("Error constructing Stripe webhook event: {:?}", e);
                Err(StatusCode::BAD_REQUEST.into_response())
            }
        }
    }
}

async fn handle_webhook(
    ctx: Ctx,
    State(state): State<Arc<CtxState>>,
    req: Request<Body>,
) -> CtxResult<Response> {
    let event = extract_stripe_event(req, &state).await?;

    let fund_service = GatewayTransactionDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    let payment_intent = match event.type_ {
        stripe::EventType::PaymentIntentSucceeded
        | stripe::EventType::PaymentIntentPartiallyFunded => {
            if let stripe::EventObject::PaymentIntent(payment_intent) = event.data.object {
                Some(payment_intent)
            } else {
                None
            }
        }
        _ => {
            if state.is_development {
                println!("Unknown event encountered in webhook: {:?}", event.type_);
            }
            None
        }
    };

    match payment_intent {
        Some(payment_intent) => {
            // TODO -fixed_decimals- amount in db should be in fixed with decimals
            let amount_received = payment_intent.amount_received / 100;
            if amount_received <= 0 {
                return Ok("No amount received".into_response());
            }

            let endowment_id: Option<EndowmentIdent> = payment_intent
                .metadata
                .get(PRODUCT_ID_KEY)
                .and_then(|pr_id| EndowmentIdent::try_from(pr_id).ok());

            let user_id: Thing = match endowment_id {
                Some(end_id) => end_id.user_id,
                None => fund_service.unknown_endowment_user_id(),
            };

            let external_account =
                payment_intent
                    .customer
                    .as_ref()
                    .map_or("unknown_customer".to_string(), |cust| match cust {
                        stripe::Expandable::Id(ref id) => id.as_str().to_string(),
                        stripe::Expandable::Object(ref obj) => obj.id.as_str().to_string(),
                    });

            let external_tx_id = payment_intent.id;

            let endowment_saved = fund_service
                .user_deposit_tx(
                    &user_id,
                    external_account,
                    external_tx_id.to_string(),
                    amount_received,
                    CurrencySymbol::USD,
                )
                .await;
            if endowment_saved.is_err() {
                println!(
                    "ERROR saving endowment user={user_id}, amount={amount_received}, stripe_tx={}",
                    external_tx_id.to_string()
                );
            }

            let _ = state
                .db
                .user_notifications
                .create(
                    &user_id.to_raw(),
                    "update balance",
                    UserNotificationEvent::UserBalanceUpdate.as_str(),
                    &vec![user_id.to_raw()],
                    None,
                )
                .await?;

            Ok("Full payment processed".into_response())
        }
        None => Ok("No valid data to process".into_response()),
    }
}
