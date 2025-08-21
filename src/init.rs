use crate::{
    entities::{
        self, community::community_entity::CommunityDbService,
        user_auth::local_user_entity::LocalUserDbService,
    },
    middleware::{ctx::Ctx, error::AppResult, mw_ctx::CtxState},
    routes::{
        auth_routes,
        community::profile_routes,
        discussions, follows, notifications, posts, reply, swagger, tags, tasks,
        user_auth::{
            login_routes, register_routes,
            webauthn::webauthn_routes::{self, WebauthnConfig},
        },
        user_otp, users, wallet,
        webhooks::paypal,
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
use entities::community::post_stream_entity::PostStreamDbService;
use entities::task::task_request_entity::TaskRequestDbService;
use entities::user_auth::authentication_entity::AuthenticationDbService;
use entities::user_auth::follow_entity::FollowDbService;
use entities::wallet::balance_transaction_entity::BalanceTransactionDbService;
use entities::wallet::lock_transaction_entity::LockTransactionDbService;
use entities::wallet::wallet_entity::WalletDbService;
use reqwest::StatusCode;
use std::sync::Arc;
use tower_cookies::CookieManagerLayer;
use tower_http::services::ServeDir;

use crate::database::client::Database;
use crate::entities::wallet::gateway_transaction_entity::GatewayTransactionDbService;

pub async fn create_default_profiles(ctx_state: &CtxState, password: &str) {
    let c = Ctx::new(Ok("create_drave_profiles".parse().unwrap()), false);

    let auth_service = AuthService::new(
        &ctx_state.db.client,
        &c,
        &ctx_state.jwt,
        &ctx_state.email_sender,
        ctx_state.verification_code_ttl,
        &ctx_state.db.verification_code,
        &ctx_state.db.access,
    );

    let _ = auth_service
        .register_password(AuthRegisterInput {
            username: "darve-starter".to_string(),
            password: password.to_string(),
            email: None,
            bio: None,
            birth_day: None,
            full_name: None,
            image_uri: None,
        })
        .await;

    let _ = auth_service
        .register_password(AuthRegisterInput {
            username: "darve-super".to_string(),
            password: password.to_string(),
            email: None,
            bio: None,
            birth_day: None,
            full_name: None,
            image_uri: None,
        })
        .await;
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
    LockTransactionDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    PostStreamDbService { db: &db, ctx: &c }.mutate_db().await?;
    GatewayTransactionDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    Ok(())
}

pub async fn main_router(ctx_state: &Arc<CtxState>, wa_config: WebauthnConfig) -> Router {
    Router::new()
        .route("/hc", get(get_hc))
        .nest_service("/assets", ServeDir::new("assets"))
        .merge(auth_routes::routes())
        .merge(login_routes::routes())
        .merge(register_routes::routes())
        .merge(posts::routes(ctx_state.upload_max_size_mb))
        .merge(webauthn_routes::routes(wa_config, "assets/wasm"))
        .merge(profile_routes::routes())
        .merge(tasks::routes())
        .merge(notifications::routes())
        .merge(users::routes(ctx_state.upload_max_size_mb))
        .merge(paypal::routes())
        .merge(discussions::routes(ctx_state.upload_max_size_mb))
        .merge(follows::routes())
        .merge(swagger::routes())
        .merge(wallet::routes(ctx_state.is_development))
        .merge(user_otp::routes())
        .merge(tags::routes())
        .merge(reply::routes())
        .with_state(ctx_state.clone())
        .layer(CookieManagerLayer::new())
}

async fn get_hc() -> Response {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    (StatusCode::OK, format!("v{}", VERSION)).into_response()
}
