use std::time::Duration;

use crate::{
    entities::{
        self, community::community_entity::CommunityDbService,
        user_auth::local_user_entity::LocalUserDbService,
    },
    middleware::{
        ctx::Ctx,
        db,
        error::{AppError, AppResult},
        mw_ctx::{self, CtxState},
    },
    routes::{
        self, auth, events, stripe_webhook_v2,
        wallet::{wallet_endowment_routes, wallet_routes},
    },
    services::auth_service::{AuthRegisterInput, AuthService},
};
use axum::{
    body::Body,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use axum_htmx::AutoVaryLayer;
use entities::community::discussion_entity::DiscussionDbService;
use entities::community::discussion_notification_entity::DiscussionNotificationDbService;
use entities::community::discussion_topic_entity::DiscussionTopicDbService;
use entities::community::post_entity::PostDbService;
use entities::community::post_stream_entity::PostStreamDbService;
use entities::community::reply_entity::ReplyDbService;
use entities::task::task_deliverable_entity::TaskDeliverableDbService;
use entities::task::task_request_entity::TaskRequestDbService;
use entities::task::task_request_participation_entity::TaskParticipationDbService;
use entities::user_auth::access_gain_action_entity::AccessGainActionDbService;
use entities::user_auth::access_right_entity::AccessRightDbService;
use entities::user_auth::access_rule_entity::AccessRuleDbService;
use entities::user_auth::authentication_entity::AuthenticationDbService;
use entities::user_auth::follow_entity::FollowDbService;
use entities::user_auth::user_notification_entity::UserNotificationDbService;
use entities::wallet::currency_transaction_entity::CurrencyTransactionDbService;
use entities::wallet::lock_transaction_entity::LockTransactionDbService;
use entities::wallet::wallet_entity::WalletDbService;
use reqwest::{header::USER_AGENT, StatusCode};
use routes::community::{
    community_routes, discussion_routes, discussion_topic_routes, post_routes, profile_routes,
    reply_routes, stripe_routes,
};
use routes::task::task_request_routes;
use routes::user_auth::webauthn::webauthn_routes::{self, WebauthnConfig};
use routes::user_auth::{
    access_gain_action_routes, access_rule_routes, follow_routes, init_server_routes, login_routes,
    register_routes, user_notification_routes,
};
use tower_cookies::CookieManagerLayer;
use tower_http::services::ServeDir;
use uuid::Uuid;

use axum::http;
use http::Request;
use tower_http::{classify::ServerErrorsFailureClass, trace::TraceLayer};
use tracing::{info, Span};

pub async fn create_default_profiles(ctx_state: &CtxState, password: &str) {
    let c = Ctx::new(
        Ok("create_drave_profiles".parse().unwrap()),
        Uuid::new_v4(),
        false,
    );

    let auth_service = AuthService::new(&ctx_state._db, &c, ctx_state.jwt.clone());

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

pub async fn run_migrations(db: db::Db) -> AppResult<()> {
    let c = Ctx::new(Ok("migrations".parse().unwrap()), Uuid::new_v4(), false);
    // let ts= TicketDbService {db: &db, ctx: &c };
    // ts.mutate_db().await?;

    LocalUserDbService { db: &db, ctx: &c }.mutate_db().await?;
    AuthenticationDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    DiscussionDbService { db: &db, ctx: &c }.mutate_db().await?;
    DiscussionTopicDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    PostDbService { db: &db, ctx: &c }.mutate_db().await?;
    ReplyDbService { db: &db, ctx: &c }.mutate_db().await?;
    DiscussionNotificationDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    CommunityDbService { db: &db, ctx: &c }.mutate_db().await?;
    AccessRuleDbService { db: &db, ctx: &c }.mutate_db().await?;
    AccessRightDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    AccessGainActionDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    FollowDbService { db: &db, ctx: &c }.mutate_db().await?;
    TaskRequestDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    TaskParticipationDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    TaskDeliverableDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    WalletDbService { db: &db, ctx: &c }.mutate_db().await?;
    CurrencyTransactionDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    LockTransactionDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    PostStreamDbService { db: &db, ctx: &c }.mutate_db().await?;
    UserNotificationDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    Ok(())
}

pub async fn main_router(ctx_state: &CtxState, wa_config: WebauthnConfig) -> Router {
    Router::new()
        .route("/hc", get(get_hc))
        .nest_service("/assets", ServeDir::new("assets"))
        // No requirements
        // Also behind /api, but no auth requirement on this route
        .merge(stripe_webhook_v2::routes(ctx_state.clone()))
        .merge(init_server_routes::routes(ctx_state.clone()))
        .merge(auth::routes(ctx_state.clone()))
        .merge(login_routes::routes(ctx_state.clone()))
        .merge(register_routes::routes(ctx_state.clone()))
        .merge(discussion_routes::routes(ctx_state.clone()))
        .merge(discussion_topic_routes::routes(ctx_state.clone()))
        .merge(community_routes::routes(ctx_state.clone()))
        .merge(access_rule_routes::routes(ctx_state.clone()))
        .merge(post_routes::routes(ctx_state.clone()))
        .merge(reply_routes::routes(ctx_state.clone()))
        .merge(webauthn_routes::routes(
            ctx_state.clone(),
            wa_config,
            "assets/wasm",
        ))
        .merge(stripe_routes::routes(ctx_state.clone()))
        .merge(access_gain_action_routes::routes(ctx_state.clone()))
        .merge(profile_routes::routes(ctx_state.clone()))
        .merge(task_request_routes::routes(ctx_state.clone()))
        .merge(follow_routes::routes(ctx_state.clone()))
        .merge(user_notification_routes::routes(ctx_state.clone()))
        .merge(wallet_routes::routes(ctx_state.clone()))
        .merge(wallet_endowment_routes::routes(ctx_state.clone()))
        .merge(events::routes(ctx_state.clone()))
        .layer(AutoVaryLayer)
        // .layer(axum::middleware::map_response(mw_req_logger))
        // .layer(middleware::map_response(mw_response_transformer::mw_htmx_transformer))
        /*.layer(middleware::from_fn_with_state(
            ctx_state.clone(),
            mw_ctx::mw_require_login,
        ))*/
        // This is where Ctx gets created, with every new request
        .layer(axum::middleware::from_fn_with_state(
            ctx_state.clone(),
            mw_ctx::mw_ctx_constructor,
        ))
        .layer(
            TraceLayer::new_for_http()
                .on_request(|request: &Request<Body>, _: &Span| {
                    let user_agent = request
                        .headers()
                        .get(USER_AGENT)
                        .map(|d| format!("{:?}", d))
                        .unwrap_or("None".to_string());

                    let ctx = request.extensions().get::<Ctx>().cloned();
                    let (req_id, user_id) = match ctx {
                        Some(v) => (v.req_id().to_string(), format!("{:?}", v.user_id())),
                        None => ("None".into(), "None".into()),
                    };

                    info!(
                        request_id = %req_id,
                        user_id = %user_id,
                        method = %request.method(),
                        uri = %request.uri().path(),
                        user_agent = &user_agent,
                        "Request"
                    );
                })
                .on_response(|response: &Response<Body>, latency: Duration, _: &Span| {
                    let status = response.status();
                    let error = response
                        .extensions()
                        .get::<AppError>()
                        .map(|e| format!("{e:?}"));

                    info!(
                        status = %status,
                        latency_ms = %latency.as_millis(),
                        error = ?error,
                        "Response"
                    );
                })
                .on_failure(
                    |error: ServerErrorsFailureClass, _: Duration, _span: &Span| {
                        tracing::debug!("something went wrong {:?}", error)
                    },
                ),
        )
        // Layers are executed from bottom up, so CookieManager has to be under ctx_constructor
        .layer(CookieManagerLayer::new())
    // .layer(Extension(ctx_state.clone()))
    // .fallback_service(routes_static());
}

async fn get_hc() -> Response {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    (StatusCode::OK, format!("v{}", VERSION)).into_response()
}
