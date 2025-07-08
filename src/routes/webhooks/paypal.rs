use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::HeaderMap,
    routing::post,
    Router,
};
use surrealdb::sql::Thing;
use uuid::Uuid;

use crate::{
    entities::wallet::gateway_transaction_entity::GatewayTransactionDbService,
    middleware::{ctx::Ctx, error::CtxResult, mw_ctx::CtxState},
    utils::paypal::{EventType, Paypal},
};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new().route("/__paypal/webhook", post(handle_webhook))
}

async fn handle_webhook(
    State(state): State<Arc<CtxState>>,
    headers: HeaderMap,
    req: Request<Body>,
) -> CtxResult<()> {
    let body = req.into_body();

    let bytes = to_bytes(body, 1024 * 1024)
        .await
        .expect("Paypal webhook event parse body err");

    let paypal = Paypal::new(
        &state.paypal_client_id,
        &state.paypal_client_key,
        &state.paypal_webhook_id,
    );
    let event = paypal
        .get_event_from_request(headers, bytes)
        .await
        .expect("Paypal get event form body error");

    let ctx = Ctx::new(
        Err(crate::middleware::error::AppError::AuthFailNoJwtCookie),
        Uuid::new_v4(),
        false,
    );

    match event.event_type {
        EventType::PaymentPayoutItemSucceeded => {
            let batch_id: &str = &event.resource.sender_batch_id.unwrap();
            let batch_thing = Thing::try_from(batch_id).expect("parse thing error");
            let db_service = GatewayTransactionDbService {
                db: &state.db.client,
                ctx: &ctx,
            };
            db_service.user_withdraw_tx_complete(batch_thing).await?;
        }
        EventType::PaymentPayoutBatchDenied => {
            let batch_header = event.resource.batch_header.unwrap();
            let batch_id = batch_header.sender_batch_header.sender_batch_id;
            let batch_thing = Thing::try_from(batch_id).expect("parse thing error");
            let db_service = GatewayTransactionDbService {
                db: &state.db.client,
                ctx: &ctx,
            };
            db_service.user_withdraw_tx_revert(batch_thing).await?;
        }
        _ => {
            let batch_id: &str = &event.resource.sender_batch_id.unwrap();
            let batch_thing = Thing::try_from(batch_id).expect("parse thing error");
            let db_service = GatewayTransactionDbService {
                db: &state.db.client,
                ctx: &ctx,
            };
            db_service.user_withdraw_tx_revert(batch_thing).await?;
        }
    }
    Ok(())
}
