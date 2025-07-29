use std::sync::Arc;

use super::error::{AppError, AppResult, CtxError, CtxResult};
use crate::middleware::mw_ctx::{CtxState, JWT_KEY};
use askama::Template;
use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, State},
    http::request::Parts,
    response::Html,
};
use axum_extra::extract::cookie::CookieJar;
use reqwest::StatusCode;
use serde::Serialize;

#[derive(Clone, Debug)]
pub struct Ctx {
    result_user_id: AppResult<String>,
    pub is_htmx: bool,
}

impl Ctx {
    pub fn new(result_user_id: AppResult<String>, is_htmx: bool) -> Self {
        Self {
            result_user_id,
            is_htmx,
        }
    }

    pub fn user_id(&self) -> CtxResult<String> {
        self.result_user_id.clone().map_err(|error| CtxError {
            error,
            is_htmx: self.is_htmx,
        })
    }

    pub fn user_thing_id(&self) -> CtxResult<String> {
        let id = self.user_id()?;
        match id.find(":") {
            None => Ok(id),
            Some(ind) => Ok((&id[ind + 1..]).to_string()),
        }
    }

    pub fn to_htmx_or_json<T: Template + Serialize>(&self, object: T) -> CtxResult<Html<String>> {
        let rendered_string = match self.is_htmx {
            true => object.render().map_err(|_| {
                self.to_ctx_error(AppError::Generic {
                    description: "Render template error".to_string(),
                })
            })?,
            false => serde_json::to_string(&object).map_err(|_| {
                self.to_ctx_error(AppError::Generic {
                    description: "Render json error".to_string(),
                })
            })?,
        };
        Ok(Html(rendered_string))
    }

    pub fn to_ctx_error(&self, error: AppError) -> CtxError {
        CtxError {
            is_htmx: self.is_htmx,
            error,
        }
    }
}

#[async_trait]
impl FromRequestParts<Arc<CtxState>> for Ctx {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<CtxState>,
    ) -> Result<Self, Self::Rejection> {
        let State(app_state): State<Arc<CtxState>> = State::from_request_parts(parts, state)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let cookies = CookieJar::from_request_parts(parts, state)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        let is_htmx = parts.headers.get("hx-request").is_some();

        let prefers_html = if !is_htmx {
            match parts.headers.get("accept").and_then(|v| v.to_str().ok()) {
                Some(accept) if accept.contains("application/json") => false,
                Some(accept) if accept.contains("text/plain") => true,
                Some(accept) if accept.contains("text/html") => true,
                Some(accept) if accept.contains("text/event-stream") => false,
                _ => true,
            }
        } else {
            true
        };

        let jwt_user_id: Result<String, AppError> = match cookies.get(JWT_KEY) {
            Some(cookie) => match app_state
                .jwt
                .decode_by_type(cookie.value(), crate::utils::jwt::TokenType::Login)
            {
                Ok(claims) => Ok(claims.auth),
                Err(_) => Err(AppError::AuthFailNoJwtCookie),
            },
            None => Err(AppError::AuthFailNoJwtCookie),
        };

        Ok(Ctx::new(jwt_user_id, prefers_html))
    }
}
