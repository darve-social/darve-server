use axum_test::{TestServer, TestServerConfig};
use chrono::Duration;
use darve_server::middleware;
use darve_server::routes::user_auth::{register_routes, webauthn};
use middleware::mw_ctx::{create_ctx_state, CtxState};
use register_routes::RegisterInput;
use serde::Deserialize;
use surrealdb::engine::any::{connect, Any};
use surrealdb::Surreal;
use webauthn::webauthn_routes::create_webauth_config;

async fn init_test_db() -> Surreal<Any> {
    let db = connect("mem://").await.unwrap();
    db.use_ns("namespace").use_db("database").await.unwrap();
    darve_server::init::run_migrations(db.clone())
        .await
        .expect("migrations run");
    db
}

// allowing this because we are importing these in test files and cargo compiler doesnt compile those files while building so skips the import of create_test_server
#[allow(dead_code)]
pub async fn create_test_server() -> (TestServer, CtxState) {
    let db = init_test_db().await;

    let ctx_state = create_ctx_state(
        db,
        "123".to_string(),
        true,
        "".to_string(),
        Duration::new(7 * 86400, 0).unwrap(),
        "".to_string(),
        "".to_string(),
        "".to_string(),
        "uploads".to_string(),
        15,
    );

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

#[derive(Deserialize, Debug)]
struct RegisterResponse {
    pub id: String,
}

#[allow(dead_code)]
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
