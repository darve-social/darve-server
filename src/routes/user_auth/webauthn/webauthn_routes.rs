use super::auth::{finish_authentication, finish_register, start_authentication, start_register};
use super::startup::AppState;
use axum::error_handling::HandleErrorLayer;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Extension, Router};
use middleware::mw_ctx::CtxState;
use std::path::PathBuf;
use tower::ServiceBuilder;
use tower_cookies::cookie::SameSite;
use tower_sessions::cookie::time::Duration;
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer};

use crate::middleware;

pub struct WebauthnConfig {
    pub relaying_party_domain: String,
    pub relaying_party_origin_url: String,
    pub is_https: bool,
    pub relaying_party_name: String,
}

pub fn create_webauth_config() -> WebauthnConfig {
    let wa_config = WebauthnConfig {
        relaying_party_domain: String::from("localhost"),
        relaying_party_origin_url: String::from("http://localhost:8080"),
        is_https: false,
        relaying_party_name: String::from("NewEra Network"),
    };
    wa_config
}

pub fn routes(state: CtxState, wa_config: WebauthnConfig, wasm_dir_path: &str) -> Router {
    let is_https = wa_config.is_https.clone();
    // Create the app
    let webauthn_state = AppState::new(wa_config);

    // TODO replace with https://github.com/rynoV/tower-sessions-surrealdb-store
    let session_store = MemoryStore::default();
    let webauthn_session_service = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|_| async { StatusCode::BAD_REQUEST }))
        /* .layer(HandleErrorLayer::new(|_: BoxError| async {
            StatusCode::BAD_REQUEST
        }))*/
        .layer(
            SessionManagerLayer::new(session_store)
                .with_name("webauthnrs")
                .with_same_site(SameSite::Strict)
                .with_secure(is_https) // TODO: change this to true when running on an HTTPS/production server instead of locally
                .with_expiry(Expiry::OnInactivity(Duration::seconds(360))),
        );

    // build our application with a route
    let webauthn_app_routes = Router::new()
        .route(
            "/api/passkey/register_start/:username",
            post(start_register),
        )
        .route("/api/passkey/register_finish", post(finish_register))
        .route(
            "/api/passkey/login_start/:username",
            post(start_authentication),
        )
        .route("/api/passkey/login_finish", post(finish_authentication))
        .layer(Extension(webauthn_state))
        .layer(webauthn_session_service)
        .fallback(handler_404);

    // #[cfg(feature = "wasm")]
    if !PathBuf::from(wasm_dir_path).exists() {
        // panic!("Can't find WASM files to serve!")
        println!("Can't find WASM files to serve!");
    }

    // #[cfg(feature = "wasm")]
    let webauthn_app_routes = Router::new().merge(webauthn_app_routes).nest_service(
        "/passkey",
        tower_http::services::ServeDir::new(wasm_dir_path),
    );

    webauthn_app_routes.with_state(state)
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "nothing to see here")
}
