pub mod community_helpers;
pub mod post_helpers;
pub mod user_helpers;
use axum_test::{TestServer, TestServerConfig};
use chrono::Duration;
use darve_server::entities::user_auth::local_user_entity::LocalUser;
use darve_server::middleware;
use darve_server::routes::user_auth::webauthn;
use dotenv::dotenv;
use fake::{faker, Fake};
use middleware::mw_ctx::{create_ctx_state, CtxState};
use serde_json::json;
use surrealdb::engine::any::{connect, Any};
use surrealdb::Surreal;
use darve_server::middleware::db;
use darve_server::middleware::db::DBConfig;
use webauthn::webauthn_routes::create_webauth_config;

async fn init_test_db(mem_db: bool) -> Surreal<Any> {
    let db = if(mem_db){
        let db = connect("mem://").await.unwrap();
        db.use_ns("namespace").use_db("database").await.unwrap();
        db
    }else {
        let config = DBConfig::from_env();
        println!("remote db config={:?}",&config);
        let db = db::start(config).await.unwrap();
        db.query("REMOVE DATABASE IF EXISTS test").await.unwrap();
        println!("remote db data reset");
        db
    };
    
    darve_server::init::run_migrations(db.clone())
        .await
        .expect("migrations run");
    db
}

// allowing this because we are importing these in test files and cargo compiler doesnt compile those files while building so skips the import of create_test_server
#[allow(dead_code)]
pub async fn create_test_server() -> (TestServer, CtxState) {
    dotenv().ok();

    let db = init_test_db(true).await;

    let ctx_state = create_ctx_state(
        db,
        "123".to_string(),
        true,
        "".to_string(),
        Duration::new(7 * 86400, 0).unwrap(),
        "".to_string(),
        "".to_string(),
        "".to_string(),
        15,
        "".to_string(),
        "".to_string(),
        10,
    )
    .await;

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
pub async fn create_fake_login_test_user(server: &TestServer) -> (&TestServer, LocalUser) {
    let pwd = faker::internet::en::Password(6..8).fake::<String>();

    let create_user = &server
        .post("/api/register")
        .json(&json!({
            "username": fake_username_min_len(6),
            "password": pwd.clone(),
            "email": Some(faker::internet::en::FreeEmail().fake::<String>()),
            "full_name": Some(faker::name::en::Name().fake::<String>()),
        }))
        .await;
    create_user.assert_status_success();
    let user = create_user.json::<LocalUser>();

    (server, user)
}

#[allow(dead_code)]
pub fn fake_username_min_len(min_len: usize) -> String {
    use fake::{faker::internet::en::Username, Fake};
    (0..)
        .map(|_| Username().fake::<String>())
        .find(|u| u.len() >= min_len)
        .unwrap()
}
