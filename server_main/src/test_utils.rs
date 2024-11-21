use sb_middleware::ctx::Ctx;
use sb_middleware::{db, mw_ctx, mw_req_logger};
use sb_middleware::mw_ctx::{create_ctx_state, CtxState};
use sb_user_auth::routes::register_routes::{register_user, RegisterInput};
use sb_user_auth::routes::webauthn::webauthn_routes::create_webauth_config;
use axum_test::{TestServer, TestServerConfig};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use axum::{middleware, Router};
use axum_htmx::AutoVaryLayer;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use tower_cookies::CookieManagerLayer;
use tower_http::services::ServeDir;
use sb_community::routes::{community_routes, discussion_routes, discussion_topic_routes, post_routes, profile_routes, reply_routes, stripe_routes};
use sb_user_auth::entity::access_right_entity::AccessRightDbService;
use sb_user_auth::entity::access_rule_entity::AccessRuleDbService;
use sb_user_auth::entity::authentication_entity::AuthenticationDbService;
use sb_community::entity::community_entitiy::CommunityDbService;
use sb_community::entity::discussion_entitiy::DiscussionDbService;
use sb_community::entity::discussion_topic_entitiy::DiscussionTopicDbService;
use sb_user_auth::entity::local_user_entity::LocalUserDbService;
use sb_community::entity::discussion_notification_entitiy::DiscussionNotificationDbService;
use sb_user_auth::entity::access_gain_action_entitiy::AccessGainActionDbService;
use sb_community::entity::post_entitiy::PostDbService;
use sb_community::entity::reply_entitiy::ReplyDbService;
use sb_user_auth::routes::*;
use sb_user_auth::routes::webauthn::webauthn_routes;
use sb_user_auth::routes::webauthn::webauthn_routes::WebauthnConfig;
use sb_middleware::error::{AppError, AppResult};
use sb_user_auth::entity::follow_entitiy::FollowDbService;
use crate::{main_router, runMigrations};

pub async fn create_test_server() -> (AppResult<TestServer>, CtxState) {
    let db = Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().to_string());
    let db_start = db::start(db).await;
    if db_start.is_err() {
        panic!("DB ERR={:?}",db_start.err().unwrap());
    }
    let is_dev = true;
    runMigrations(db_start.unwrap(), is_dev).await.expect("migrations run");

    let ctx_state = create_ctx_state("123".to_string(), is_dev, "".to_string() , "".to_string() , "".to_string(), "uploads".to_string());
    let wa_config = create_webauth_config();
    let routes_all = main_router(&ctx_state.clone(), wa_config).await;

    let server = TestServer::new_with_config(
        routes_all,
        TestServerConfig { transport: None, save_cookies: true, expect_success_by_default: false, restrict_requests_with_http_schema: false, default_content_type: None, default_scheme: None })
        .map_err(|e| AppError::Generic {description:format!("server did not start err{}", e)});
    (server, ctx_state)
}

#[derive(Deserialize, Debug)]
struct RegisterResponse {
    pub id: String,
}
pub async fn create_login_test_user(server: &TestServer, username: String) -> (&TestServer, String) {
    let create_user = &server.post("/api/register").json(&RegisterInput { username: username.to_string() , password: "some3242paSs#$".to_string(), email:None, next: None, password1: "some3242paSs#$".to_string() }).await;
    create_user.assert_status_success();
    // dbg!(&create_user);
    // let userId: String = create_user;
    let registered = &create_user.json::<RegisterResponse>();
    // let login_user = &server.post("/api/login").json(&LoginInput { username: username.to_string(), password: "some3242paSs#$".to_string(), next: None }).await;
    // login_user.assert_status_success();

    (server, registered.id.clone())
}


pub async fn create_dev_env(ctx_state: &CtxState, username: String, pass: String) {
    let ctx = &Ctx::new(Ok(username.clone().to_string()), Uuid::new_v4(), false);
    let admin = register_user(&ctx_state._db, &ctx, &RegisterInput { username: username.clone().to_string(), password: pass.clone(), password1: pass.clone(),  email: None, next: None }).await.unwrap();
    let user = register_user(&ctx_state._db, &ctx, &RegisterInput { username: "test".to_string(), password: "test123".to_string(), password1: "test123".to_string(), email: None, next: None }).await.unwrap();

}
