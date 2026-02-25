use std::sync::Arc;

use axum::{
    extract::{FromRequestParts, State},
    http::request::Parts,
};
use axum_extra::headers::{authorization::Bearer, Authorization, HeaderMapExt};
use reqwest::StatusCode;

use crate::{
    middleware::{ctx::Ctx, mw_ctx::CtxState},
    utils::jwt::TokenType,
};

pub struct BearerAuth {
    pub user_id: String,
    pub ctx: Ctx,
}

impl BearerAuth {
    pub fn user_thing_id(&self) -> String {
        match self.user_id.find(":") {
            None => self.user_id.clone(),
            Some(ind) => (&self.user_id[ind + 1..]).to_string(),
        }
    }
}

impl FromRequestParts<Arc<CtxState>> for BearerAuth {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<CtxState>,
    ) -> Result<Self, Self::Rejection> {
        let State(app_state): State<Arc<CtxState>> = State::from_request_parts(parts, state)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match parts.headers.typed_get::<Authorization<Bearer>>() {
            Some(token) => match app_state
                .jwt
                .decode_by_type(token.token(), TokenType::Login)
            {
                Ok(claims) => Ok(BearerAuth {
                    user_id: claims.auth.clone(),
                    ctx: Ctx::new(Ok(claims.auth), false),
                }),
                Err(_) => Err(StatusCode::UNAUTHORIZED),
            },
            _ => Err(StatusCode::UNAUTHORIZED),
        }
    }
}
