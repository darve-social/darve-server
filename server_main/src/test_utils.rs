use crate::{main_router, run_migrations};
use axum_test::{TestServer, TestServerConfig};
use chrono::Duration;
use sb_middleware::ctx::Ctx;
use sb_middleware::error::CtxResult;
use sb_middleware::mw_ctx::{create_ctx_state, CtxState};
use sb_user_auth::routes::register_routes::{register_user, RegisterInput};
use sb_user_auth::routes::webauthn::webauthn_routes::create_webauth_config;
use serde::Deserialize;
use surrealdb::engine::any::{connect, Any};
use surrealdb::Surreal;
use uuid::Uuid;
use sb_middleware::utils::db_utils::{IdentIdName};
use sb_user_auth::entity::local_user_entity::LocalUserDbService;


async fn init_test_db() -> Surreal<Any> {
    let db = connect("mem://").await.unwrap();
    db.use_ns("namespace").use_db("database").await.unwrap();
    run_migrations(db.clone()).await.expect("migrations run");
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

pub async fn create_dev_env(
    ctx_state: &CtxState,
    username: String,
    pass: String,
    email: Option<String>,
    bio: Option<String>,
    image_uri: Option<String>,
    full_name: Option<String>,
) -> CtxResult<Vec<String>> {
    let ctx = &Ctx::new(Ok(username.clone().to_string()), Uuid::new_v4(), false);
    let user_ser = LocalUserDbService{ db: &ctx_state._db, ctx };

    let exists = user_ser.get(IdentIdName::ColumnIdent {
        column: "username".to_string(),
        val: "test0".to_string(),
        rec: false,
    }).await;

    if exists.is_ok() {
        return Ok(vec![]);
    }

    let hardcoded_bios =
        [
            ("💥 Hero-in-training with explosive ambition to be #0! 💣", "https://static0.gamerantimages.com/wordpress/wp-content/uploads/2023/02/shigaraki-face.jpg"),
            ("🥇 Champ-in-training with explosive ambition to be #1! 💣", "https://fanboydestroy.com/wp-content/uploads/2019/04/ary-and-the-secret-of-seasons-super-resolution-2019.03.22-11.55.42.73.png"),
            ("‼️ QA-in-training with explosive ambition to be #2! 💣", "https://static0.gamerantimages.com/wordpress/wp-content/uploads/2022/07/Genshin-Impact-Sumeru-region.jpg"),
             ("👾 BOT-in-training with explosive ambition to be #3! 💣", "https://static0.gamerantimages.com/wordpress/wp-content/uploads/2023/10/cocoon-container-creature.jpg"),

        ];

    let reg_inputs: Vec<RegisterInput> = hardcoded_bios.iter().enumerate().map(|i_bio|{
        let username = format!("test{}", i_bio.0);
        RegisterInput{
            username: username.clone(),
            password: "000000".to_string(),
            password1: "000000".to_string(),
            email: Some(format!("{}@email.com", username.as_str())),
            bio: Some(i_bio.1.0.to_string()),
            full_name: Some(format!("User {username}")),
            image_uri: Some(i_bio.1.1.to_string()),
            next: None,
        }
    }).collect();

    let id0 = register_user(&ctx_state._db, &ctx, &reg_inputs[0]).await.unwrap().id;
    let id1 = register_user(&ctx_state._db, &ctx, &reg_inputs[1]).await.unwrap().id;
    let id2 = register_user(&ctx_state._db, &ctx, &reg_inputs[2]).await.unwrap().id;
    let id3 = register_user(&ctx_state._db, &ctx, &reg_inputs[3] ).await.unwrap().id;

    // create one more user with the input data

    let id4 = register_user(
        &ctx_state._db,
        &ctx,
        &RegisterInput {
            username: username.clone().to_string(),
            password: pass.clone().to_string(),
            password1: pass.clone().to_string(),
            email: email.clone(),
            bio: bio.clone(),
            full_name: full_name.clone(),
            image_uri: image_uri.clone(),
            next: None,
        },
    )
    .await
    .unwrap()
    .id;
    Ok(vec![id0, id1, id2, id3, id4])
}
