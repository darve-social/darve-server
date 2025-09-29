use std::sync::Arc;

use axum::{
    extract::{FromRequestParts, State},
    http::request::Parts,
};
use axum_extra::extract::CookieJar;
use reqwest::StatusCode;

use super::ctx::Ctx;
use crate::database::surrdb_utils::get_thing_id;
use crate::{
    middleware::mw_ctx::{CtxState, JWT_KEY},
    utils::jwt::TokenType,
};

#[derive(Debug)]
pub struct AuthWithLoginAccess {
    pub user_id: String,
    pub ctx: Ctx,
}

impl AuthWithLoginAccess {
    pub fn user_thing_id(&self) -> String {
        get_thing_id(self.user_id.as_str()).to_string()
    }
}

impl FromRequestParts<Arc<CtxState>> for AuthWithLoginAccess {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<CtxState>,
    ) -> Result<Self, Self::Rejection> {
        let State(app_state): State<Arc<CtxState>> = State::from_request_parts(parts, state)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let cookies = CookieJar::from_headers(&parts.headers);

        match cookies.get(JWT_KEY) {
            Some(cookie) => match app_state
                .jwt
                .decode_by_type(cookie.value(), TokenType::Login)
            {
                Ok(claims) => Ok(AuthWithLoginAccess {
                    user_id: claims.auth.clone(),
                    ctx: Ctx::new(Ok(claims.auth), false),
                }),
                Err(_) => Err(StatusCode::UNAUTHORIZED),
            },
            _ => Err(StatusCode::UNAUTHORIZED),
        }
    }
}
