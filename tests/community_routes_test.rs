mod helpers;

use access_right_entity::AccessRightDbService;
use authorization_entity::{Authorization, AUTH_ACTIVITY_OWNER};
use axum::http::HeaderValue;
use axum_htmx::HX_REQUEST;
use community_entity::CommunityDbService;
use community_routes::CommunityInput;
use darve_server::entities::community::community_entity;
use darve_server::entities::user_auth::{access_right_entity, authorization_entity};
use darve_server::middleware;
use darve_server::routes::community::community_routes;
use helpers::create_test_server;
use middleware::ctx::Ctx;
use middleware::utils::db_utils::IdentIdName;
use middleware::utils::request_utils::CreatedResponse;
use serial_test::serial;
use surrealdb::sql::Thing;
use uuid::Uuid;

use crate::helpers::create_fake_login_test_user;

#[tokio::test]
#[serial]
async fn get_community_view() {
    let (server, ctx_state) = create_test_server().await;
    let (server, _, _) = create_fake_login_test_user(&server).await;
    let disc_name = "discName1".to_lowercase();

    let create_response = server
        .post("/api/community")
        .json(&CommunityInput {
            id: "".to_string(),
            name_uri: disc_name.clone(),
            title: "The Community Test".to_string(),
        })
        .add_header("Accept", "application/json")
        .await;
    let created = create_response.json::<CreatedResponse>();
    // dbg!(&created);

    let comm_id = Thing::try_from(created.id.clone()).unwrap();
    let comm_uri = created.uri.clone().unwrap();

    create_response.assert_status_success();

    let comm_db = CommunityDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false),
    };
    let comm_rec = comm_db.get(IdentIdName::Id(comm_id)).await;
    assert_eq!(comm_rec.clone().unwrap().default_discussion.is_some(), true);

    let comm_rec = comm_db
        .get(IdentIdName::ColumnIdent {
            val: comm_uri.clone(),
            column: "name_uri".to_string(),
            rec: false,
        })
        .await;
    assert_eq!(comm_rec.clone().unwrap().default_discussion.is_some(), true);

    let get_response = server
        .get(format!("/community/{comm_uri}").as_str())
        .add_header("Accept", "application/json")
        .await;
    get_response.assert_status_success();
    dbg!(get_response);
}

#[tokio::test]
#[serial]
async fn create_community() {
    let (server, ctx_state) = create_test_server().await;
    let (server, user, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();

    let comm_name = "discName1".to_lowercase();
    let comm_name2 = "disc_name2".to_lowercase();

    let create_response = server
        .post("/api/community")
        .json(&CommunityInput {
            id: "".to_string(),
            name_uri: comm_name.clone(),
            title: "The Community".to_string(),
        })
        .add_header("Accept", "application/json")
        .await;

    let created = &create_response.json::<CreatedResponse>();
    // dbg!(&created);
    let comm_name_created = created.uri.clone().unwrap();
    assert_eq!(comm_name, comm_name_created);
    let comm_id1 = Thing::try_from(created.id.clone()).unwrap();

    create_response.assert_status_success();

    // same username should return error
    let create_response2 = server
        .post("/api/community")
        .json(&CommunityInput {
            id: "".to_string(),
            name_uri: comm_name_created.clone(),
            title: "The Community2".to_string(),
        })
        .add_header("Accept", "application/json")
        .await;
    // dbg!(&create_response2);
    create_response2.assert_status_bad_request();

    let create_response2 = server
        .post("/api/community")
        .json(&CommunityInput {
            id: "".to_string(),
            name_uri: comm_name2.clone(),
            title: "The Community2".to_string(),
        })
        .add_header("Accept", "application/json")
        .await;

    create_response2.assert_status_success();
    let created_comm2 = create_response2.json::<CreatedResponse>();
    let comm2_id = Thing::try_from(created_comm2.id).unwrap();

    let ctx1 = &Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);
    let comm_db = CommunityDbService {
        db: &ctx_state.db.client,
        ctx: ctx1,
    };

    let comm = comm_db.get(IdentIdName::Id(comm_id1).into()).await;
    let comm_by_uri = comm_db
        .get(IdentIdName::ColumnIdent {
            column: "name_uri".to_string(),
            val: comm_name_created.to_string(),
            rec: false,
        })
        .await;
    let community1 = comm.unwrap();
    let community1_by_uri = comm_by_uri.unwrap();
    assert_eq!(community1.clone().name_uri, comm_name_created.clone());
    assert_eq!(
        community1_by_uri.clone().name_uri,
        comm_name_created.clone()
    );
    let _ = community1.default_discussion.clone().unwrap();

    let comm2 = comm_db.get(IdentIdName::Id(comm2_id.clone()).into()).await;
    let comm_by_uri2 = comm_db
        .get(IdentIdName::ColumnIdent {
            column: "name_uri".to_string(),
            val: comm_name2.to_string(),
            rec: false,
        })
        .await;
    let community2 = comm2.unwrap();
    let community2_by_uri = comm_by_uri2.unwrap();
    assert_eq!(community2.clone().name_uri, comm_name2.clone());
    assert_eq!(community2_by_uri.clone().name_uri, comm_name2.clone());
    let _ = community2.default_discussion.clone().unwrap();

    let db_service = AccessRightDbService {
        db: &ctx_state.db.client,
        ctx: ctx1,
    };
    let user_auth = db_service
        .get_authorizations(&Thing::try_from(user_ident.clone()).unwrap())
        .await
        .unwrap();

    let user_auth1 = user_auth.get(1).unwrap();

    assert_eq!(
        user_auth1.authorize_record_id.eq(&community1.id.unwrap()),
        true
    );
    let user_auth2 = user_auth.get(2).unwrap();
    assert_eq!(user_auth2.authorize_record_id.eq(&comm2_id), true);

    println!("uuu0= {:?} ", user_auth.clone());

    assert_eq!(user_auth.clone().len(), 3);

    let smaller_auth = Authorization {
        authorize_record_id: community2.clone().id.unwrap(),
        authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
        authorize_height: 98,
    };
    let higher_auth = Authorization {
        authorize_record_id: community2.id.unwrap(),
        authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
        authorize_height: 100,
    };

    assert_eq!(
        smaller_auth
            .ge(&higher_auth, ctx1, &ctx_state.db.client)
            .await
            .is_err(),
        false
    );
    let found: Vec<Authorization> = user_auth
        .clone()
        .into_iter()
        .filter(|v| v.ge_equal_ident(&smaller_auth).ok().unwrap_or(false))
        .collect();
    assert_eq!(found.len(), 1);
    let found: Vec<Authorization> = user_auth
        .into_iter()
        .filter(|v| v.ge_equal_ident(&higher_auth).ok().unwrap_or(false))
        .collect();
    assert_eq!(found.len(), 0);

    let community_resp = server
        .get(format!("/community/{comm_name_created}").as_str())
        .add_header(HX_REQUEST, HeaderValue::from_static("true"))
        .add_header("Accept", "application/json")
        .await;

    // dbg!(&community_resp);
    community_resp.assert_status_success();

    let community_resp = server
        .get(
            format!("/community/{comm_name_created}?topic_id=community_topic:A01JXJXV9YQWFAS1V8GTHFAYY24")
                .as_str(),
        )
        .add_header(HX_REQUEST, HeaderValue::from_static("true"))
        .add_header("Accept", "application/json")
        .await;

    // dbg!(&community_resp);
    community_resp.assert_status_success();
}
