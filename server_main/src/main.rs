extern crate dotenv;

use std::net::{Ipv4Addr, SocketAddr};

use axum::{middleware, Router};
use axum::http::{ StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum_htmx::AutoVaryLayer;
use chrono::Duration;
use dotenv::dotenv;
use error::AppResult;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use tokio;
use tower_cookies::CookieManagerLayer;
use tower_http::services::ServeDir;
use uuid::Uuid;

use sb_user_auth::routes::webauthn::webauthn_routes;

use crate::test_utils::create_dev_env;
use sb_community::entity::community_entitiy::CommunityDbService;
use sb_community::entity::discussion_entitiy::DiscussionDbService;
use sb_community::entity::discussion_notification_entitiy::DiscussionNotificationDbService;
use sb_community::entity::discussion_topic_entitiy::DiscussionTopicDbService;
use sb_community::entity::post_entitiy::PostDbService;
use sb_community::entity::post_stream_entitiy::PostStreamDbService;
use sb_community::entity::reply_entitiy::ReplyDbService;
use sb_community::routes::{
    community_routes, discussion_routes, discussion_topic_routes, post_routes, profile_routes,
    reply_routes, stripe_routes,
};
use sb_middleware::ctx::Ctx;
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::{db, error, mw_ctx, mw_req_logger};
use sb_task::entity::task_deliverable_entitiy::TaskDeliverableDbService;
use sb_task::entity::task_request_entitiy::TaskRequestDbService;
use sb_task::entity::task_request_offer_entity::TaskRequestOfferDbService;
use sb_task::routes::task_request_routes;
use sb_user_auth::entity::access_gain_action_entitiy::AccessGainActionDbService;
use sb_user_auth::entity::access_right_entity::AccessRightDbService;
use sb_user_auth::entity::access_rule_entity::AccessRuleDbService;
use sb_user_auth::entity::authentication_entity::AuthenticationDbService;
use sb_user_auth::entity::follow_entitiy::FollowDbService;
use sb_user_auth::entity::local_user_entity::LocalUserDbService;
use sb_user_auth::entity::user_notification_entitiy::UserNotificationDbService;
use sb_user_auth::routes::webauthn::webauthn_routes::WebauthnConfig;
use sb_user_auth::routes::{access_gain_action_routes, access_rule_routes, follow_routes, init_server_routes, login_routes, register_routes, user_notification_routes};
use sb_wallet::entity::currency_transaction_entitiy::CurrencyTransactionDbService;
use sb_wallet::entity::wallet_entitiy::WalletDbService;
use sb_wallet::routes::{wallet_routes, wallet_endowment_routes};

mod mw_response_transformer;
mod test_utils;
mod tests;

#[tokio::main]
async fn main() -> AppResult<()> {
    dotenv().ok();
    let is_dev = std::env::var("DEVELOPMENT")
        .expect("set DEVELOPMENT env var")
        .eq("true");
    let init_server_password = std::env::var("START_PASSWORD").expect("password to start request");
    let stripe_key = std::env::var("STRIPE_SECRET_KEY").expect("Missing STRIPE_SECRET_KEY in env");
    let stripe_wh_secret =
        std::env::var("STRIPE_WEBHOOK_SECRET").expect("Missing STRIPE_WEBHOOK_SECRET in env");
    let stripe_platform_account =
        std::env::var("STRIPE_PLATFORM_ACCOUNT").expect("Missing STRIPE_PLATFORM_ACCOUNT in env");
    let uploads_dir = std::env::var("UPLOADS_DIRECTORY").expect("Missing UPLOADS_DIRECTORY in env");
    let jwt_secret = std::env::var("JWT_SECRET").expect("Missing JWT_SECRET in env");
    let jwt_duration = Duration::days(7);

    let db = db::start(None).await?;
    run_migrations(db, is_dev).await?;

    let ctx_state = mw_ctx::create_ctx_state(
        init_server_password,
        is_dev,
        jwt_secret,
        jwt_duration,
        stripe_key,
        stripe_wh_secret,
        stripe_platform_account,
        uploads_dir,
    );
    let wa_config = webauthn_routes::create_webauth_config();
    let routes_all = main_router(&ctx_state, wa_config).await;

    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8080));
    println!("->> LISTENING on {addr}\n");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    println!("DEVELOPMENT={}", is_dev);
    if ctx_state.is_development {
        let username = "userrr".to_string();
        let password = "password".to_string();
        let email = "dynamite@myheroacademia.io".to_string();
        let bio = "💥 Hero-in-training with explosive ambition to be #1! 💣".to_string();
        let full_name = "Katsuki Bakugo".to_string();
        let image_uri =
            "https://qph.cf2.quoracdn.net/main-qimg-64a32df103bc8fb7b2fc495553a5fc0a-lq"
                .to_string();
        create_dev_env(
            &ctx_state.clone(),
            username.clone(),
            password.clone(),
            Some(email.clone()),
            Some(bio.clone()),
            Some(image_uri.clone()),
            Some(full_name.clone()),
        )
        .await;
        open::that(format!(
            "http://localhost:8080/login?u={username}&p={password}"
        ))
        .expect("browser opens");
    }

    axum::serve(listener, routes_all.into_make_service())
        .await
        .unwrap();

    // fallback fs
    // fn routes_static() -> Router {
    //     Router::new().nest_service("/", get_service(ServeDir::new("./")))
    // }

    Ok(())
}

async fn run_migrations(db: Surreal<Db>, is_development: bool) -> AppResult<()> {
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
    TaskRequestOfferDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    TaskDeliverableDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    WalletDbService {
        db: &db,
        ctx: &c,
    }
    .mutate_db()
    .await?;
    CurrencyTransactionDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    PostStreamDbService { db: &db, ctx: &c }.mutate_db().await?;
    UserNotificationDbService { db: &db, ctx: &c }.mutate_db().await?;
    Ok(())
}

pub async fn main_router(ctx_state: &CtxState, wa_config: WebauthnConfig) -> Router {
    Router::new()
        .route("/hc", get(get_hc))
        .nest_service("/assets", ServeDir::new("./server_main/src/assets"))
        // No requirements
        // Also behind /api, but no auth requirement on this route
        .merge(init_server_routes::routes(ctx_state.clone()))
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
            "./server_main/src/assets/wasm",
        ))
        .merge(stripe_routes::routes(ctx_state.clone()))
        .merge(access_gain_action_routes::routes(ctx_state.clone()))
        .merge(profile_routes::routes(ctx_state.clone()))
        .merge(task_request_routes::routes(ctx_state.clone()))
        .merge(follow_routes::routes(ctx_state.clone()))
        .merge(user_notification_routes::routes(ctx_state.clone()))
        .merge(user_notification_routes::routes(ctx_state.clone()))
        .merge(wallet_routes::routes(ctx_state.clone()))
        .merge(wallet_endowment_routes::routes(ctx_state.clone()))
        // .merge(file_upload_routes::routes(ctx_state.clone(), ctx_state.uploads_dir.as_str()).await)
        .layer(AutoVaryLayer)
        .layer(middleware::map_response(mw_req_logger::mw_req_logger))
        // .layer(middleware::map_response(mw_response_transformer::mw_htmx_transformer))
        /*.layer(middleware::from_fn_with_state(
            ctx_state.clone(),
            mw_ctx::mw_require_login,
        ))*/
        // This is where Ctx gets created, with every new request
        .layer(middleware::from_fn_with_state(
            ctx_state.clone(),
            mw_ctx::mw_ctx_constructor,
        ))
        // Layers are executed from bottom up, so CookieManager has to be under ctx_constructor
        .layer(CookieManagerLayer::new())
    // .layer(Extension(ctx_state.clone()))
    // .fallback_service(routes_static());
}

async fn get_hc()-> Response {
    (StatusCode::OK, "v0.0.1".to_string()).into_response()
}