use crate::{main_router, runMigrations};
use axum_test::{TestServer, TestServerConfig};
use sb_middleware::ctx::Ctx;
use sb_middleware::error::{AppError, AppResult};
use sb_middleware::mw_ctx::{create_ctx_state, CtxState};
use sb_middleware::db;
use sb_user_auth::routes::register_routes::{register_user, RegisterInput};
use sb_user_auth::routes::webauthn::webauthn_routes::create_webauth_config;
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub async fn create_test_server() -> (AppResult<TestServer>, CtxState) {
    let db = Some(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string(),
    );
    let db_start = db::start(db).await;
    if db_start.is_err() {
        panic!("DB ERR={:?}", db_start.err().unwrap());
    }
    let is_dev = true;
    runMigrations(db_start.unwrap(), is_dev)
        .await
        .expect("migrations run");

    let ctx_state = create_ctx_state(
        "123".to_string(),
        is_dev,
        "".to_string(),
        "".to_string(),
        "".to_string(),
        "uploads".to_string(),
    );
    let wa_config = create_webauth_config();
    let routes_all = main_router(&ctx_state.clone(), wa_config).await;

    let server = TestServer::new_with_config(
        routes_all,
        TestServerConfig {
            transport: None,
            save_cookies: true,
            expect_success_by_default: false,
            restrict_requests_with_http_schema: false,
            default_content_type: None,
            default_scheme: None,
        },
    )
    .map_err(|e| AppError::Generic {
        description: format!("server did not start err{}", e),
    });
    (server, ctx_state)
}

#[derive(Deserialize, Debug)]
struct RegisterResponse {
    pub id: String,
}
pub async fn create_login_test_user(
    server: &TestServer,
    username: String,
) -> (&TestServer, String) {
    let create_user = &server
        .post("/api/register")
        .json(&RegisterInput {
            username: username.to_string(),
            password: "some3242paSs#$".to_string(),
            email: None,
            next: None,
            password1: "some3242paSs#$".to_string(),
            bio: None,
            full_name: None,
            image_uri: None,
        })
        .await;
    create_user.assert_status_success();
    // dbg!(&create_user);
    // let userId: String = create_user;
    let registered = &create_user.json::<RegisterResponse>();
    // let login_user = &server.post("/api/login").json(&LoginInput { username: username.to_string(), password: "some3242paSs#$".to_string(), next: None }).await;
    // login_user.assert_status_success();

    (server, registered.id.clone())
}

pub async fn create_dev_env(
    ctx_state: &CtxState,
    username: String,
    pass: String,
    email: Option<String>,
    bio: Option<String>,
    image_uri: Option<String>,
    full_name: Option<String>,
) {
    let ctx = &Ctx::new(Ok(username.clone().to_string()), Uuid::new_v4(), false);
    let admin = register_user(
        &ctx_state._db,
        &ctx,
        &RegisterInput {
            username: username.clone().to_string(),
            password: pass.clone(),
            password1: pass.clone(),
            email: email.clone(),
            bio: bio.clone(),
            full_name: full_name.clone(),
            image_uri: image_uri.clone(),
            next: None,
        },
    )
    .await
    .unwrap();
    let user = register_user(&ctx_state._db, &ctx, &RegisterInput { username: "test".to_string(), password: "test123".to_string(), password1: "test123".to_string(), email: None,bio:None, full_name:Some("Shigaraki".to_string()),image_uri:Some("https://static0.gamerantimages.com/wordpress/wp-content/uploads/2023/02/shigaraki-face.jpg".to_string()),next: None }).await.unwrap();
}
