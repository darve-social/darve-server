use std::sync::Arc;

use askama_axum::axum_core::response::IntoResponse;
use axum::body::Body;
use axum::extract::{FromRef, FromRequest, Request, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::post;
use axum::{async_trait, Router};
use gateway_transaction_entity::GatewayTransactionDbService;
use serde::Serialize;
use stripe::Event;
use surrealdb::sql::Thing;
use wallet_entity::CurrencySymbol;

use crate::entities::user_notification::UserNotificationEvent;
use crate::entities::wallet::{gateway_transaction_entity, wallet_entity};
use crate::interfaces::repositories::user_notifications::UserNotificationsInterface;
use crate::middleware;
use crate::middleware::mw_ctx::CtxState;
use crate::middleware::utils::extractor_utils::extract_stripe_event;
use crate::middleware::utils::string_utils::get_str_thing;
use middleware::ctx::Ctx;
use middleware::error::CtxResult;

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new().route("/__stripe/webhook", post(handle_webhook))
}

#[derive(Debug, Serialize)]
struct StripeEvent(Event);

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

            let user_id: Thing = match payment_intent.metadata.get("user_id") {
                Some(id) => get_str_thing(id).expect("Parse user_id stripe webhook"),
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
