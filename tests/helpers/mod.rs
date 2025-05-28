pub mod community_helpers;
pub mod post_helpers;
pub mod user_helpers;
use axum_test::{TestServer, TestServerConfig};
use chrono::Duration;
use darve_server::entities::user_auth::local_user_entity::LocalUser;
use darve_server::middleware;
use darve_server::routes::user_auth::{register_routes, webauthn};
use fake::{faker, Fake};
use middleware::mw_ctx::{create_ctx_state, CtxState};
use register_routes::RegisterInput;
use serde::Deserialize;
use surrealdb::engine::any::{connect, Any};
use surrealdb::sql::Thing;
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
        "".to_string(),
        "".to_string(),
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

#[allow(dead_code)]
pub async fn create_fake_login_test_user(server: &TestServer) -> (&TestServer, LocalUser) {
    let pwd = faker::internet::en::Password(6..8).fake::<String>();

    let input = RegisterInput {
        username: fake_username_min_len(7),
        password: pwd.clone(),
        email: Some(faker::internet::en::FreeEmail().fake::<String>()),
        next: None,
        password1: pwd.clone(),
        bio: None,
        full_name: Some(faker::name::en::Name().fake::<String>()),
        image_uri: None,
    };

    let create_user = &server.post("/api/register").json(&input).await;
    create_user.assert_status_success();
    let registered = &create_user.json::<RegisterResponse>();
    let user = LocalUser {
        id: Some(Thing::try_from(registered.id.clone()).unwrap()),
        username: input.username,
        full_name: input.full_name,
        birth_date: None,
        phone: None,
        email: input.email,
        bio: input.bio,
        social_links: None,
        image_uri: input.image_uri,
        email_verified: None,
    };

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
