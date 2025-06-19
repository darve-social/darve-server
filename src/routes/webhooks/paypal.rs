use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::HeaderMap,
    routing::post,
    Router,
};

use crate::{
    middleware::{error::CtxResult, mw_ctx::CtxState},
    utils::paypal::Paypal,
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

    let bytes = to_bytes(body, 1)
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

    match event.event_type.as_str() {
        "PAYMENT.PAYOUTS-ITEM.SUCCEEDED" => {
            println!("✅ Payout succeeded: {:?}", event.resource);
        }
        "PAYMENT.PAYOUTS-ITEM.UNCLAIMED" => {
            println!("⏳ Payout unclaimed: {:?}", event.resource);
        }
        "PAYMENT.PAYOUTS-ITEM.RETURNED" => {
            println!("↩️ Payout returned: {:?}", event.resource);
        }
        _ => {
            println!("ℹ️ Unhandled event: {:?}", event.event_type);
        }
    }

    Ok(())
}
