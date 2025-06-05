use axum::{
    body::Body,
    extract::{FromRequest, State},
    http::Request,
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use reqwest::StatusCode;

use crate::{
    interfaces::payment::PaymentInterface,
    middleware::{
        ctx::Ctx,
        error::{AppError, CtxResult},
        mw_ctx::{CtxState, StripeConfig},
    },
    services::wallet::WalletService,
    utils::stripe::{
        models::EventType,
        stripe::StripePayment,
        webhook::{event::HookEvent, hooks::verify_and_parse_event},
    },
};
use async_trait::async_trait;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/_____/v2/stripe/webhook", post(webhook))
        .with_state(state)
}

#[async_trait]
impl<S> FromRequest<S> for HookEvent
where
    String: FromRequest<S>,
    S: Send + Sync + StripeConfig,
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

        let wh_secret = state.get_webhook_secret();

        let event = verify_and_parse_event(&payload, &signature.to_str().unwrap(), &wh_secret)
            .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;

        Ok(event)
    }
}

async fn webhook(
    State(state): State<CtxState>,
    ctx: Ctx,
    hook_event: HookEvent,
) -> CtxResult<Response> {
    let stripe = Box::new(StripePayment::new(state.stripe_secret_key.clone()));

    let event = stripe
        .get_event(&hook_event.id)
        .await
        .map_err(|e| AppError::Stripe { source: e })?;

    match event.event_type {
        EventType::AccountLinkCompleted => {
            let account_id = event.data.get("account_id").ok_or(AppError::Stripe {
                source: format!("Invalid event data {:?}", event.event_type),
            })?;

            let wallet_service = WalletService::new(&state._db, &ctx, stripe);

            let _ = wallet_service
                .withdraw(&account_id.as_str().unwrap())
                .await
                .map_err(|e| e.to_string());
        }
        EventType::OutboundPaymentCanceled => {
            todo!()
        }
        EventType::OutboundPaymentFailed => {
            todo!()
        }
        EventType::OutboundPaymentReturned => {
            todo!()
        }
        EventType::OutboundPaymentPosted => {
            todo!()
        }
    }

    Ok("".into_response())
}
