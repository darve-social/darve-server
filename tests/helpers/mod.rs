pub mod community_helpers;
pub mod post_helpers;
pub mod user_helpers;
use std::sync::Arc;

use axum_test::{TestServer, TestServerConfig};
use darve_server::config::AppConfig;
use darve_server::database::client::{Database, DbConfig};
use darve_server::entities::user_auth::local_user_entity::LocalUser;
use darve_server::middleware;
use darve_server::routes::user_auth::webauthn;
use fake::{faker, Fake};
use middleware::mw_ctx::{create_ctx_state, CtxState};
use serde_json::json;
use webauthn::webauthn_routes::create_webauth_config;

#[allow(dead_code)]
async fn init_test_db(config: &mut AppConfig) -> Database {
    println!("remote db config={:?}", &config);
    config.db_database = "darve_test".to_string();
    // config.db_url = "mem://".to_string();
    // config.db_password = None;
    // config.db_username = None;
    let db = Database::connect(DbConfig {
        url: &config.db_url,
        database: &config.db_database,
        namespace: &config.db_namespace,
        password: config.db_password.as_deref(),
        username: config.db_username.as_deref(),
    })
    .await;

    db.client
        .query(
            "DELETE FROM 
                access_right,
                access_rule,
                authentication,
                balance_transaction,
                local_user,
                wallet,
                gateway_transaction,
                lock_transaction,
                transaction_head,
                user_notification,
                follow,
                post_stream,
                join_action,
                community,
                task_deliverable,
                task_request,
                task_request_participation,
                post,
                like,
                discussion_topic,
                verification_code,
                notification;",
        )
        .await
        .unwrap();

    db.run_migrations().await.unwrap();

    darve_server::init::run_migrations(&db.client)
        .await
        .expect("migrations run");

    db
}

// allowing this because we are importing these in test files and cargo compiler doesnt compile those files while building so skips the import of create_test_server
#[allow(dead_code)]
pub async fn create_test_server() -> (TestServer, Arc<CtxState>) {
    let mut config = AppConfig::from_env();

    let db = init_test_db(&mut config).await;
    let ctx_state = create_ctx_state(db, &config).await;

    let wa_config = create_webauth_config();
    let routes_all = darve_server::init::main_router(&ctx_state.clone(), wa_config).await;

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
    .expect("Failed to create test server");

    (server, ctx_state)
}

#[allow(dead_code)]
pub async fn create_login_test_user(
    server: &TestServer,
    username: String,
) -> (&TestServer, String) {
    let create_user = &server
        .post("/api/register")
        .json(&json!({ "username": username.to_string(), "password": "some3242paSs#$".to_string()}))
        .await;

    println!("Creating user with username: {username} {:?}", create_user);
    create_user.assert_status_success();
    let registered = create_user.json::<LocalUser>();
    (server, registered.id.unwrap().to_raw())
}

#[allow(dead_code)]
pub async fn create_fake_login_test_user(server: &TestServer) -> (&TestServer, LocalUser, String) {
    let pwd = faker::internet::en::Password(6..8).fake::<String>();
    let username = fake_username_min_len(6);
    let create_user = &server
        .post("/api/register")
        .json(&json!({
            "username": username,
            "password": pwd.clone(),
            "email": Some(faker::internet::en::FreeEmail().fake::<String>()),
            "full_name": Some(faker::name::en::Name().fake::<String>()),
        }))
        .await;

    create_user.assert_status_success();
    let user = create_user.json::<LocalUser>();

    (server, user, pwd)
}

#[allow(dead_code)]
pub fn fake_username_min_len(min_len: usize) -> String {
    use fake::{faker::internet::en::Username, Fake};
    (0..)
        .map(|_| Username().fake::<String>().replace(".", "_"))
        .find(|u| u.len() >= min_len)
        .unwrap()
}
