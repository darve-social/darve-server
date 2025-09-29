use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use gateway_transaction_entity::GatewayTransactionDbService;
use surrealdb::sql::Thing;
use wallet_entity::CurrencySymbol;

use crate::entities::wallet::{gateway_transaction_entity, wallet_entity};
use crate::middleware;
use crate::middleware::error::AppError;
use crate::middleware::mw_ctx::CtxState;
use crate::middleware::utils::extractor_utils::extract_stripe_event;
use crate::middleware::utils::string_utils::get_str_thing;
use crate::services::notification_service::NotificationService;
use middleware::ctx::Ctx;
use middleware::error::CtxResult;

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new().route("/__stripe/webhook", post(handle_webhook))
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
            let amount_received = payment_intent.amount_received;
            if amount_received <= 0 {
                return Ok("No amount received".into_response());
            }

            let tx_id = payment_intent
                .metadata
                .get("tx_id")
                .ok_or(AppError::Generic {
                    description: "gateway transaction id not found".to_string(),
                })?;

            let gateway_id = get_str_thing(&tx_id)?;

            let user_id: Thing = match payment_intent.metadata.get("user_id") {
                Some(id) => get_str_thing(id).expect("Parse user_id stripe webhook"),
                None => fund_service.unknown_endowment_user_id(),
            };

            let external_tx_id = payment_intent.id;

            let endowment_saved = fund_service
                .user_deposit_tx(
                    gateway_id,
                    external_tx_id.to_string(),
                    amount_received,
                    CurrencySymbol::USD,
                    None,
                )
                .await;
            if endowment_saved.is_err() {
                println!(
                    "ERROR saving endowment user={user_id}, amount={amount_received}, stripe_tx={}",
                    external_tx_id.to_string()
                );
            }

            let notification_service = NotificationService::new(
                &state.db.client,
                &ctx,
                &state.event_sender,
                &state.db.user_notifications,
            );

            let _ = notification_service.on_completed_deposit(&user_id).await;
            let _ = notification_service.on_update_balance(&user_id).await;

            Ok("Full payment processed".into_response())
        }
        None => Ok("No valid data to process".into_response()),
    }
}
