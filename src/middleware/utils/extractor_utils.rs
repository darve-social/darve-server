use axum::body::Body;
use axum::extract::{self, FromRequest, Request};
use axum::http::header::CONTENT_TYPE;
use axum::http::StatusCode;
use axum::{
    async_trait,
    response::{IntoResponse, Response},
    Form, Json, RequestExt,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use stripe::Event;
use validator::{Validate, ValidationErrors};

use crate::middleware::error::{to_err_html, AppError, ErrorResponseBody};
use crate::middleware::mw_ctx::CtxState;

#[derive(Debug)]
pub struct JsonOrFormValidated<T>(pub T);

#[async_trait]
impl<S, T> FromRequest<S> for JsonOrFormValidated<T>
where
    S: Send + Sync,
    Json<T>: FromRequest<()>,
    Form<T>: FromRequest<()>,
    T: DeserializeOwned + Validate + Send + Sync + 'static,
{
    type Rejection = Response;

    async fn from_request(req: Request<Body>, _state: &S) -> Result<Self, Self::Rejection> {
        let content_type_header = req.headers().get(CONTENT_TYPE);
        let content_type = content_type_header.and_then(|value| value.to_str().ok());

        if let Some(content_type) = content_type {
            if content_type.starts_with("application/json") {
                let Json(payload) = req.extract().await.map_err(IntoResponse::into_response)?;
                let validation: Result<(), ValidationErrors> = payload.validate();
                validation.map_err(|err| {
                    {
                        let body: String = ErrorResponseBody::new(err.to_string(), None).into();
                        (StatusCode::BAD_REQUEST, body)
                    }
                    .into_response()
                })?;
                return Ok(Self(payload));
            }

            if content_type.starts_with("application/x-www-form-urlencoded") {
                // htmx request
                let Form(payload) = req.extract().await.map_err(IntoResponse::into_response)?;
                payload.validate().map_err(|err| {
                    { (StatusCode::BAD_REQUEST, to_err_html(err.to_string())) }.into_response()
                })?;
                return Ok(Self(payload));
            }
        }

        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct DiscussionParams {
    pub start: Option<u32>,
    pub count: Option<u16>,
}

pub async fn extract_stripe_event(req: Request<Body>, state: &CtxState) -> Result<Event, AppError> {
    let (parts, body) = req.into_parts();
    let headers = &parts.headers.clone();

    let signature = headers
        .get("stripe-signature")
        .ok_or_else(|| AppError::Stripe {
            source: "Missing Stripe signature".to_string(),
        })?
        .to_str()
        .map_err(|e| AppError::Stripe {
            source: e.to_string(),
        })?;

    let req = Request::from_parts(parts, body);

    let payload: String =
        req.extract()
            .await
            .map_err(|e: extract::rejection::StringRejection| AppError::Stripe {
                source: e.to_string(),
            })?;
    let event = stripe::Webhook::construct_event(&payload, signature, &state.stripe_wh_secret)
        .map_err(|e| AppError::Stripe {
            source: e.to_string(),
        })?;
    Ok(event)
}
