use crate::{
    entities::{
        self,
        community::community_entity::CommunityDbService,
        user_auth::local_user_entity::{LocalUserDbService, UserRole},
    },
    middleware::{
        ctx::Ctx,
        error::{AppError, AppResult},
        mw_ctx::CtxState,
    },
    routes::{
        admin, auth_routes,
        community::profile_routes,
        discussions, editor_tags, follows, notifications, posts, reply, swagger, tags, tasks,
        user_auth::{
            login_routes, register_routes,
            webauthn::webauthn_routes::{self, WebauthnConfig},
        },
        user_otp, users, wallet,
        webhooks::{paypal, stripe},
    },
    services::auth_service::{AuthRegisterInput, AuthService},
};
use axum::{
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use entities::community::discussion_entity::DiscussionDbService;
use entities::community::post_entity::PostDbService;
use entities::task::task_request_entity::TaskRequestDbService;
use entities::user_auth::authentication_entity::AuthenticationDbService;
use entities::user_auth::follow_entity::FollowDbService;
use entities::wallet::balance_transaction_entity::BalanceTransactionDbService;
use entities::wallet::wallet_entity::WalletDbService;
use reqwest::StatusCode;
use std::sync::Arc;
use std::time::Duration;
use tower_cookies::CookieManagerLayer;
use tower_http::{classify::ServerErrorsFailureClass, services::ServeDir, trace::TraceLayer};
use tracing::{debug, error, info, warn};

use crate::database::client::Database;
use crate::entities::wallet::gateway_transaction_entity::GatewayTransactionDbService;

pub async fn create_default_profiles(ctx_state: &CtxState, password: &str) {
    let c = Ctx::new(Ok("create_drave_profiles".parse().unwrap()), false);

    let auth_service = AuthService::new(
        &ctx_state.db.client,
        &c,
        &ctx_state.jwt,
        ctx_state.email_sender.clone(),
        ctx_state.verification_code_ttl,
        &ctx_state.db.verification_code,
        &ctx_state.db.access,
        ctx_state.file_storage.clone(),
    );

    let _ = auth_service
        .register_password(
            AuthRegisterInput {
                username: "darve".to_string(),
                password: password.to_string(),
                email: None,
                bio: None,
                birth_day: None,
                full_name: None,
                image: None,
            },
            Some(UserRole::Admin),
        )
        .await
        .map_err(|e| {
            println!(">>>>>>>{:?}", e);
            e
        });
}

pub async fn run_migrations(database: &Database) -> AppResult<()> {
    let db = database.client.clone();
    let c = Ctx::new(Ok("migrations".parse().unwrap()), false);

    LocalUserDbService { db: &db, ctx: &c }.mutate_db().await?;
    AuthenticationDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    DiscussionDbService { db: &db, ctx: &c }.mutate_db().await?;
    PostDbService { db: &db, ctx: &c }.mutate_db().await?;
    CommunityDbService { db: &db, ctx: &c }.mutate_db().await?;
    FollowDbService { db: &db, ctx: &c }.mutate_db().await?;
    TaskRequestDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    WalletDbService { db: &db, ctx: &c }.mutate_db().await?;
    BalanceTransactionDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    GatewayTransactionDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    Ok(())
}

pub fn main_router(
    ctx_state: &Arc<CtxState>,
    wa_config: WebauthnConfig,
    // rate_limit_rsp: u32,
    // rate_limit_burst: u32,
) -> Router {
    Router::new()
        .route("/hc", get(get_hc))
        .nest_service("/assets", ServeDir::new("assets"))
        .merge(auth_routes::routes())
        .merge(login_routes::routes())
        .merge(register_routes::routes())
        .merge(posts::routes())
        .merge(webauthn_routes::routes(wa_config, "assets/wasm"))
        .merge(profile_routes::routes())
        .merge(tasks::routes())
        .merge(notifications::routes())
        .merge(users::routes())
        .merge(paypal::routes())
        .merge(stripe::routes())
        .merge(discussions::routes(ctx_state.upload_max_size_mb))
        .merge(follows::routes())
        .merge(swagger::routes())
        .merge(wallet::routes(ctx_state.is_development))
        .merge(user_otp::routes())
        .merge(tags::routes())
        .merge(editor_tags::routes())
        .merge(reply::routes())
        .merge(admin::routes())
        .with_state(ctx_state.clone())
        .layer(CookieManagerLayer::new())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    let random = rand::random::<u32>();
                    let connection_id = format!("conn_{random:x}");
                    tracing::info_span!(
                        "http_request",
                        connection_id = %connection_id,
                        method = %request.method(),
                        uri = %request.uri(),
                    )
                })
                .on_request(|request: &axum::http::Request<_>, _span: &tracing::Span| {
                    debug!(
                        method = %request.method(),
                        uri = %request.uri(),
                        "HTTP request started"
                    );
                })
                .on_response(
                    |response: &axum::http::Response<_>,
                     latency: Duration,
                     _span: &tracing::Span| {
                        let status = response.status();
                        if status.is_client_error() || status.is_server_error() {
                            let error_msg = response
                                .extensions()
                                .get::<AppError>()
                                .map_or("".to_string(), |e| e.to_string());
                            warn!(
                                status = %status,
                                error_msg = %error_msg,
                                latency_ms = latency.as_millis(),
                                "HTTP request failed"
                            );
                        } else {
                            info!(
                                status = %status,
                                latency_ms = latency.as_millis(),
                                "HTTP request completed"
                            );
                        }
                    },
                )
                .on_failure(
                    |error: ServerErrorsFailureClass, latency: Duration, _span: &tracing::Span| {
                        error!(
                            latency = ?latency,
                            error = ?error,
                            "request failed"
                        );
                    },
                ),
        )
    // .layer(create_rate_limit_layer(rate_limit_rsp, rate_limit_burst))
}

async fn get_hc() -> Response {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    (StatusCode::OK, format!("v{}", VERSION)).into_response()
}
