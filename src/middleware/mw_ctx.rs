use crate::entities::user_auth::user_notification_entity::UserNotificationEvent;
use crate::middleware::{ctx::Ctx, error::AppError, error::AppResult};
use crate::routes::community::community_routes::DiscussionNotificationEvent;
use crate::utils::jwt::JWT;
use axum::body::Body;
use axum::http::header::ACCEPT;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{extract::State, http::Request, middleware::Next, response::Response};
use axum_htmx::HxRequest;
use chrono::Duration;
use jsonwebtoken::{decode, DecodingKey, EncodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_cookies::{Cookie, Cookies};
use tower_http::services::ServeDir;
use uuid::Uuid;

use super::db;

#[derive(Debug, Clone, Serialize)]
pub enum AppEventType {
    UserNotificationEvent(UserNotificationEvent),
    DiscussionNotificationEvent(DiscussionNotificationEvent),
}
#[derive(Debug, Clone, Serialize)]
pub struct AppEvent {
    pub user_id: String,
    pub content: Option<String>,
    pub event: AppEventType,
    pub receivers: Vec<String>,
}

#[derive(Clone)]
pub struct CtxState {
    pub _db: db::Db,
    pub key_enc: EncodingKey,
    pub key_dec: DecodingKey,
    pub jwt_duration: Duration,
    pub start_password: String,
    pub is_development: bool,
    pub stripe_secret_key: String,
    pub stripe_wh_secret: String,
    pub stripe_platform_account: String,
    pub min_platform_fee_abs_2dec: i64,
    pub platform_fee_rel: f64,
    pub upload_max_size_mb: u64,
    pub uploads_dir: String,
    pub uploads_serve_dir: ServeDir,
    pub mobile_client_id: String,
    pub google_client_id: String,
    pub event_sender: broadcast::Sender<AppEvent>,
    pub jwt: Arc<JWT>,
}

impl Debug for CtxState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("CTX STATE HERE :)")
    }
}

pub trait StripeConfig {
    fn get_webhook_secret(&self) -> String;
}

impl StripeConfig for CtxState {
    fn get_webhook_secret(&self) -> String {
        self.stripe_wh_secret.clone()
    }
}

pub fn create_ctx_state(
    db: db::Db,
    start_password: String,
    is_development: bool,
    jwt_secret: String,
    jwt_duration: Duration,
    stripe_secret_key: String,
    stripe_wh_secret: String,
    stripe_platform_account: String,
    uploads_dir: String,
    upload_max_size_mb: u64,
    mobile_client_id: String,
    google_client_id: String,
) -> CtxState {
    let secret = jwt_secret.as_bytes();
    let key_enc = EncodingKey::from_secret(secret);
    let key_dec = DecodingKey::from_secret(secret);
    let (event_sender, _) = broadcast::channel(100);
    let ctx_state = CtxState {
        _db: db,
        key_enc,
        key_dec,
        start_password,
        is_development,
        stripe_secret_key,
        stripe_wh_secret,
        stripe_platform_account,
        jwt_duration,
        min_platform_fee_abs_2dec: 500,
        platform_fee_rel: 0.05,
        uploads_serve_dir: ServeDir::new(uploads_dir.clone())
            .append_index_html_on_directories(false),
        uploads_dir,
        upload_max_size_mb,
        jwt: Arc::new(JWT::new(jwt_secret, jwt_duration)),
        mobile_client_id,
        google_client_id,
        event_sender,
    };
    ctx_state
}

pub const JWT_KEY: &str = "jwt";

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub exp: usize,
    pub auth: String,
}

pub async fn mw_ctx_constructor(
    State(CtxState { _db, key_dec, .. }): State<CtxState>,
    cookies: Cookies,
    HxRequest(is_htmx): HxRequest,
    headers: HeaderMap,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let is_htmx = if !is_htmx {
        match headers.get(ACCEPT).map(|x| x.as_bytes()) {
            Some(b"application/json") => false,
            Some(b"text/plain") => true,
            Some(b"text/html") => true,
            // leave as it is for sse
            Some(b"text/event-stream") => false,
            _ => true,
        }
    } else {
        true
    };

    let uuid = Uuid::new_v4();
    let jwt_user_id: AppResult<String> = get_jwt_user_id(key_dec, &cookies);

    // Store Ctx in the request extension, for extracting in rest handlers
    let ctx = Ctx::new(jwt_user_id.clone(), uuid, is_htmx);
    /* removed dep to LocalUserDbService and moved to each handler
    let user_id: Option<String> = jwt_user_id.ok();
    if let Some(uid) = user_id {
        // TODO create check against cache first or remove for each request - maybe add to login request or save local_user to ctx
        let exists = LocalUserDbService { db: &_db, ctx: &ctx }.exists(IdentIdName::Id(uid)).await;
        // dbg!(&exists);
        if !exists.is_ok() || exists.unwrap_or(None).is_none() {
            cookies.remove(Cookie::from(JWT_KEY));
            return (StatusCode::NOT_FOUND, "User not found").into_response();
        }
    }*/

    req.extensions_mut().insert(ctx);

    next.run(req).await
}

pub async fn mw_require_login(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    req: Request<Body>,
    next: Next,
) -> Response {
    if ctx.user_id().is_err() {
        return (StatusCode::FORBIDDEN, "Login required").into_response();
    };
    next.run(req).await
}

pub fn get_jwt_user_id(key: DecodingKey, cookies: &Cookies) -> AppResult<String> {
    extract_token_user_id(key, cookies).map_err(|err| {
        // Remove an invalid cookie
        if let AppError::AuthFailJwtInvalid { .. } = err {
            cookies.remove(Cookie::from(JWT_KEY))
        }
        return err;
    })
}

fn verify_token(key: DecodingKey, token: &str) -> AppResult<String> {
    Ok(decode::<Claims>(token, &key, &Validation::default())?
        .claims
        .auth)
}

fn extract_token_user_id(key: DecodingKey, cookies: &Cookies) -> AppResult<String> {
    cookies
        .get(JWT_KEY)
        .ok_or(AppError::AuthFailNoJwtCookie)
        .and_then(|cookie| verify_token(key, cookie.value()))
}

#[cfg(test)]
mod tests {
    use crate::middleware::mw_ctx::Claims;
    use chrono::{Duration, Utc};
    use jsonwebtoken::{
        decode, encode, errors::ErrorKind, DecodingKey, EncodingKey, Header, Validation,
    };

    const SECRET: &[u8] = b"some-secret";
    const SOMEONE: &str = "someone";
    // cspell:disable-next-line
    const TOKEN_EXPIRED: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJleHAiOjEsImF1dGgiOiJzb21lb25lIn0.XXHVHu2IsUPA175aQ-noWbQK4Wu-2prk3qTXjwaWBvE";

    #[test]
    fn jwt_sign_expired() {
        let my_claims = Claims {
            exp: 1,
            auth: SOMEONE.to_string(),
        };
        let token_str = encode(
            &Header::default(),
            &my_claims,
            &EncodingKey::from_secret(SECRET),
        )
        .unwrap();
        assert_eq!(token_str, TOKEN_EXPIRED);
    }

    #[test]
    fn jwt_verify_expired_ignore() {
        let mut validation = Validation::default();
        validation.validate_exp = false;
        let token = decode::<Claims>(
            TOKEN_EXPIRED,
            &DecodingKey::from_secret(SECRET),
            &validation,
        )
        .unwrap();
        assert_eq!(token.claims.auth, SOMEONE);
    }

    #[test]
    fn jwt_verify_expired_fail() {
        let token_result = decode::<Claims>(
            TOKEN_EXPIRED,
            &DecodingKey::from_secret(SECRET),
            &Validation::default(),
        );
        assert!(token_result.is_err());
        let kind = token_result.map_err(|e| e.into_kind()).err();
        assert_eq!(kind, Some(ErrorKind::ExpiredSignature));
    }

    #[test]
    fn jwt_sign_and_verify_with_chrono() {
        let exp = Utc::now() + Duration::minutes(1);
        let my_claims = Claims {
            exp: exp.timestamp() as usize,
            auth: SOMEONE.to_string(),
        };
        // Sign
        let token_str = encode(
            &Header::default(),
            &my_claims,
            &EncodingKey::from_secret(SECRET),
        )
        .unwrap();
        // Verify
        let token_result = decode::<Claims>(
            &token_str,
            &DecodingKey::from_secret(SECRET),
            &Validation::default(),
        )
        .unwrap();
        assert_eq!(token_result.claims.auth, SOMEONE);
    }
}
