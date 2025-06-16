mod helpers;
use darve_server::entities::user_auth::{access_right_entity, local_user_entity};
use darve_server::middleware;
use darve_server::routes::user_auth::init_server_routes;
use serial_test::serial;
use uuid::Uuid;

use crate::helpers::create_test_server;
use access_right_entity::AccessRightDbService;
use init_server_routes::InitServerData;
use local_user_entity::LocalUserDbService;
use middleware::{ctx::Ctx, utils::db_utils::UsernameIdent};

#[tokio::test]
#[serial]
async fn init_server() {
    let (server, ctx_state) = create_test_server().await;
    // let (server,user_ident) = create_login_test_user(&server).await;
    let username = "username".to_string();
    let create_response = server
        .post("/init")
        .json(&InitServerData {
            init_pass: "12dfas3".to_string(),
            username: username.clone(),
            password: "passs43flksfalsffas3".to_string(),
            email: "emm@us.com".to_string(),
        })
        .await;
    // dbg!(&create_response);
    create_response.assert_status_success();

    let ctx = &Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);
    let user = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx,
    }
    .get(UsernameIdent(username.clone()).into())
    .await;
    let user = user.unwrap();
    // dbg!(&user);
    let access_rights = AccessRightDbService {
        db: &ctx_state.db.client,
        ctx,
    }
    .list_by_user(&user.id.unwrap())
    .await;
    assert_eq!(access_rights.unwrap().len() > 0, true);
}
