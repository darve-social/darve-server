use std::sync::Arc;

use axum::{
    async_trait,
    extract::{FromRequestParts, State},
    http::request::Parts,
};
use axum_extra::headers::{authorization::Bearer, Authorization, HeaderMapExt};
use reqwest::StatusCode;

use crate::middleware::mw_ctx::CtxState;

pub struct AuthData {
    pub user_id: String,
}
impl AuthData {
    pub fn user_thing_id(&self) -> String {
        match self.user_id.find(":") {
            None => self.user_id.clone(),
            Some(ind) => (&self.user_id[ind + 1..]).to_string(),
        }
    }
}

#[async_trait]
impl FromRequestParts<Arc<CtxState>> for AuthData {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<CtxState>,
    ) -> Result<Self, Self::Rejection> {
        let State(app_state): State<Arc<CtxState>> = State::from_request_parts(parts, state)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match parts.headers.typed_get::<Authorization<Bearer>>() {
            Some(token) => match app_state.jwt.decode(token.token()) {
                Ok(claims) => Ok(AuthData {
                    user_id: claims.auth,
                }),
                Err(_) => Err(StatusCode::UNAUTHORIZED),
            },
            _ => Err(StatusCode::UNAUTHORIZED),
        }
    }
}
