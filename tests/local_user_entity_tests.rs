mod helpers;

use darve_server::entities::user_auth::local_user_entity;
use darve_server::middleware;
use surrealdb::sql::Thing;


use crate::helpers::create_login_test_user;
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::utils::db_utils::{IdentIdName, UsernameIdent};
use middleware::utils::string_utils::get_string_thing;

test_with_server!(user_query, |server, ctx_state, config| {
    let username = "usn1ame".to_string();
    let (_, uid) = create_login_test_user(&server, username.clone()).await;

    let db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok("user_ident".parse().unwrap()), false),
    };
    let user = db_service.get(UsernameIdent(username.clone()).into()).await;
    let user = user.unwrap();
    assert_eq!(user.username, username.clone());

    let user = db_service
        .get(IdentIdName::Id(
            get_string_thing(uid.clone()).expect("thing"),
        ))
        .await;
    let user = user.unwrap();
    assert_eq!(user.username, username.clone());

    let user = db_service
        .get(IdentIdName::Id(Thing::from(("local_user", "not_existing"))))
        .await;
    assert_eq!(user.is_err(), true);
});

test_with_server!(test_exists, |server, ctx_state, config| {
    let username = "usn1ame".to_string();
    let (_, uid) = create_login_test_user(&server, username.clone()).await;

    let db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok("user_ident".parse().unwrap()), false),
    };
    let user = db_service
        .exists(UsernameIdent(username.clone()).into())
        .await;
    let user_id = user.unwrap().unwrap();
    assert_eq!(user_id, uid);

    let user = db_service
        .exists(UsernameIdent("not_exists".to_string()).into())
        .await;
    let user = user.unwrap();
    assert_eq!(user.is_some(), false);
});
