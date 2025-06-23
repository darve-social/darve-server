mod helpers;

use axum_test::multipart::MultipartForm;
use darve_server::entities::community::{community_entity, discussion_entity, post_entity};
use darve_server::entities::user_auth::{
    access_right_entity, authorization_entity, local_user_entity,
};
use darve_server::middleware;
use darve_server::routes::community::{
    community_routes, discussion_routes, discussion_topic_routes,
};
use darve_server::services::discussion_service::CreateDiscussion;
use serde_json::json;
use serial_test::serial;
use surrealdb::sql::Thing;
use uuid::Uuid;

use access_right_entity::AccessRightDbService;
use authorization_entity::{Authorization, AUTH_ACTIVITY_OWNER};
use community_entity::CommunityDbService;
use community_routes::CommunityInput;
use discussion_entity::{Discussion, DiscussionDbService};
use discussion_routes::DiscussionPostView;
use discussion_topic_routes::TopicInput;
use helpers::create_test_server;
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::error::CtxResult;
use middleware::utils::db_utils::IdentIdName;
use middleware::utils::extractor_utils::DiscussionParams;
use middleware::utils::request_utils::CreatedResponse;
use middleware::utils::string_utils::get_string_thing;
use post_entity::PostDbService;

use crate::helpers::create_fake_login_test_user;

#[tokio::test]
#[serial]
async fn get_discussion_view() {
    let (server, ctx_state) = create_test_server().await;
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();
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
    let _ = created.uri.clone().unwrap();

    create_response.assert_status_success();

    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.to_raw(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: None,
            private_discussion_users_final: false,
        })
        .add_header("Accept", "application/json")
        .await;

    let created = create_response.json::<Discussion>();
    // dbg!(&created);

    let disc_id = created.id.as_ref().unwrap();
    create_response.assert_status_success();

    let topic_resp = server
        .post(
            format!(
                "/api/discussion/{}/topic",
                created.id.as_ref().unwrap().to_raw()
            )
            .as_str(),
        )
        .json(&TopicInput {
            id: "".to_string(),
            title: "topic1".to_string(),
            hidden: None,
            access_rule_id: "".to_string(),
        })
        .add_header("Accept", "application/json")
        .await;
    dbg!(&topic_resp);
    topic_resp.assert_status_success();

    let disc_rec = DiscussionDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok(user_ident), Uuid::new_v4(), false),
    }
    .get(IdentIdName::Id(disc_id.clone()))
    .await;
    assert_eq!(disc_rec.clone().unwrap().topics.unwrap().len(), 1);
    let topics = disc_rec.unwrap().topics.unwrap();
    let topic_id = topics[0].clone();

    let post_name = "post title Name 1".to_string();
    let create_post = server
        .post(format!("/api/discussion/{disc_id}/post").as_str())
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "contentttt111")
                .add_text("topic_id", ""),
        )
        .add_header("Accept", "application/json")
        .await;
    create_post.assert_status_success();

    let post_name2 = "post title Name 2?&$^%! <>end".to_string();
    let create_response2 = server
        .post(format!("/api/discussion/{disc_id}/post").as_str())
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name2.clone())
                .add_text("content", "contentttt222")
                .add_text("topic_id", topic_id.clone().to_raw()),
        )
        .add_header("Accept", "application/json")
        .await;
    create_response2.assert_status_success();

    let disc_posts = PostDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false),
    }
    .get_by_discussion_desc_view::<DiscussionPostView>(
        disc_id.clone(),
        DiscussionParams {
            topic_id: None,
            start: None,
            count: None,
        },
    )
    .await;
    let disc_posts_top1 = PostDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false),
    }
    .get_by_discussion_desc_view::<DiscussionPostView>(
        disc_id.clone(),
        DiscussionParams {
            topic_id: Some(topic_id),
            start: None,
            count: None,
        },
    )
    .await;
    assert_eq!(disc_posts.is_ok(), true);
    assert_eq!(disc_posts.unwrap().len(), 2);
    assert_eq!(disc_posts_top1.is_ok(), true);
    assert_eq!(disc_posts_top1.unwrap().len(), 1);
}

#[tokio::test]
#[serial]
async fn create_discussion() {
    let (server, ctx_state) = create_test_server().await;
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();

    let comm_name_uri = "CommName1".to_lowercase();

    let create_response = server
        .post("/api/community")
        .json(&CommunityInput {
            id: "".to_string(),
            name_uri: comm_name_uri.clone(),
            title: "The Community Test".to_string(),
        })
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<CreatedResponse>();

    let comm_id = Thing::try_from(created.id.clone()).unwrap();

    create_response.assert_status_success();

    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.to_raw(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: None,
            private_discussion_users_final: false,
        })
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<Discussion>();
    // dbg!(&created);
    let disc_name = created.title.clone();
    assert_eq!(disc_name, Some("The Discussion".to_string()));

    create_response.assert_status_success();

    // same username should return error
    // let create_response2 = server.post("/api/discussion").json(&DiscussionInput { id: None, community_id: comm_id.to_raw(), title: "The Discussion2".to_string(), topics: None }).await;

    // dbg!(&create_response2);
    // &create_response2.assert_status_bad_request();

    // let create_response2 = server.post("/api/discussion").json(&DiscussionInput { id: None, community_id: comm_id.to_raw(), title: "The Discussion22".to_string(), topics: None }).await;
    // &create_response2.assert_status_success();
    // let created2 = &create_response2.json::<CreatedResponse>();
    // let disc2_id = created2.id.clone();

    let ctx = &Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);
    let comm_db = CommunityDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };
    let comm = comm_db.get(IdentIdName::Id(comm_id.clone())).await;
    let comm_disc_id = comm.unwrap().default_discussion.unwrap();

    let disc_db = DiscussionDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };

    let disc = disc_db
        .get(IdentIdName::Id(created.id.as_ref().unwrap().clone()).into())
        .await;
    let comm_disc: CtxResult<Discussion> = disc_db.get(IdentIdName::Id(comm_disc_id)).await;
    assert_eq!(comm_disc.unwrap().belongs_to.eq(&comm_id.clone()), true);
    // let disc_by_uri = disc_db.get(IdentIdName::ColumnIdent { column: "name_uri".to_string(), val: disc_name.to_string(), rec: false}).await;
    let discussion = disc.unwrap();
    // let discussion_by_uri = disc_by_uri.unwrap();
    assert_eq!(discussion.clone().topics, None);
    // assert_eq!(discussion.clone().name_uri.unwrap(), disc_name.clone());
    // assert_eq!(discussion_by_uri.clone().name_uri.unwrap(), disc_name.clone());

    let db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };
    let aright_db_service = AccessRightDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };
    let _ = db_service
        .get(IdentIdName::Id(get_string_thing(user_ident.clone()).expect("thing")).into())
        .await;

    let smaller_auth = Authorization {
        authorize_record_id: discussion.clone().id.unwrap(),
        authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
        authorize_height: 98,
    };
    let higher_auth = Authorization {
        authorize_record_id: discussion.id.unwrap(),
        authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
        authorize_height: 100,
    };

    assert_eq!(
        smaller_auth
            .ge(&higher_auth, ctx, &ctx_state.db.client)
            .await
            .is_err(),
        false
    );

    let mut found: Vec<Authorization> = vec![];
    let user_auth = aright_db_service
        .get_authorizations(&Thing::try_from(user_ident).unwrap())
        .await
        .expect("user authorizations");
    for v in user_auth.clone() {
        let is_ge = v.ge(&smaller_auth, ctx, &ctx_state.db.client).await;
        if is_ge.is_ok() {
            found.push(v);
        }
    }
    assert_eq!(found.len(), 2);

    let mut found: Vec<Authorization> = vec![];
    for v in user_auth.clone() {
        let is_ge = v.ge(&higher_auth, ctx, &ctx_state.db.client).await;
        if is_ge.unwrap() {
            found.push(v);
        }
    }
    assert_eq!(found.len(), 0);

    // let discussion_resp = server.get(format!("/discussion/{disc_name}").as_str()).add_header(HX_REQUEST, HeaderValue::from_static("true")).await;
    // let discussion_resp = server.get(format!("/discussion/{disc_id}").as_str()).add_header(HX_REQUEST, HeaderValue::from_static("true")).await;

    // dbg!(&discussion_resp);
    // &discussion_resp.assert_status_success();

    // let discussion_resp = server.get(format!("/discussion/{disc_id}?topic_id=discussion_topic:345").as_str()).add_header(HX_REQUEST, HeaderValue::from_static("true")).await;

    // dbg!(&discussion_resp);
    // &discussion_resp.assert_status_success();
}

#[tokio::test]
#[serial]
async fn create_chat_discussion() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, _) = create_fake_login_test_user(&server).await;

    let (server, user2, _, _) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user2.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id,
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Discussion>();

    let private_discussion_user_ids = result.private_discussion_user_ids.unwrap();
    assert_eq!(private_discussion_user_ids.len(), 2);
    assert!(private_discussion_user_ids.contains(&user1.id.as_ref().unwrap().clone()));
    assert!(private_discussion_user_ids.contains(&user2.id.as_ref().unwrap().clone()));
}

#[tokio::test]
#[serial]
async fn try_to_create_the_same_read_only() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, _) = create_fake_login_test_user(&server).await;

    let (server, user2, _, _) = create_fake_login_test_user(&server).await;
    let comm_id = CommunityDbService::get_profile_community_id(&user1.id.as_ref().unwrap().clone());
    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.clone().to_raw(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_forbidden();

    let comm_id = CommunityDbService::get_profile_community_id(&user2.id.as_ref().unwrap().clone());
    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.clone().to_raw(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Discussion>();

    let private_discussion_user_ids = result.private_discussion_user_ids.unwrap();
    assert_eq!(private_discussion_user_ids.len(), 2);
    assert!(private_discussion_user_ids.contains(&user1.id.as_ref().unwrap().clone()));
    assert!(private_discussion_user_ids.contains(&user2.id.as_ref().unwrap().clone()));

    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.clone().to_raw(),
            title: "The Discussion1".to_string(),
            image_uri: None,
            chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();

    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.to_raw(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result_1 = create_response.json::<Discussion>();
    assert_eq!(result.id, result_1.id)
}

#[tokio::test]
#[serial]
async fn try_to_create_the_same_not_read_only() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, _) = create_fake_login_test_user(&server).await;

    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user2.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Cookie", format!("jwt={}", token2))
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Discussion>();

    let private_discussion_user_ids = result.private_discussion_user_ids.unwrap();
    assert_eq!(private_discussion_user_ids.len(), 2);
    assert!(private_discussion_user_ids.contains(&user1.id.as_ref().unwrap().clone()));
    assert!(private_discussion_user_ids.contains(&user2.id.as_ref().unwrap().clone()));

    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion1".to_string(),
            image_uri: None,
            chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Cookie", format!("jwt={}", token2))
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();

    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id,
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: false,
        })
        .add_header("Cookie", format!("jwt={}", token2))
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result_1 = create_response.json::<Discussion>();
    assert_ne!(result.id, result_1.id)
}

#[tokio::test]
#[serial]
async fn get_discussions() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user2.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();

    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion1".to_string(),
            image_uri: None,
            chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();

    let create_response = server
        .get("/api/discussions")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token2))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<Discussion>>();
    assert_eq!(result.len(), 2);

    let create_response = server
        .get("/api/discussions")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<Discussion>>();
    assert_eq!(result.len(), 2)
}

#[tokio::test]
#[serial]
async fn try_add_chat_users_into_read_only() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user1.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", token1))
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user2.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Discussion>();

    let private_discussion_user_ids = result.private_discussion_user_ids.unwrap();
    assert_eq!(private_discussion_user_ids.len(), 2);
    assert!(private_discussion_user_ids.contains(&user1.id.as_ref().unwrap().clone()));
    assert!(private_discussion_user_ids.contains(&user2.id.as_ref().unwrap().clone()));

    let create_response = server
        .post(&format!(
            "/api/discussions/{}/chat_users",
            result.id.as_ref().unwrap().to_raw().replace(":", "%3A")
        ))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .json(&json!({ "user_ids": [] }))
        .await;

    create_response.assert_status_failure();

    assert!(create_response.text().contains("no users present"));

    let create_response = server
        .post(&format!(
            "/api/discussions/{}/chat_users",
            result.id.as_ref().unwrap().to_raw().replace(":", "%3A")
        ))
        .add_header("Cookie", format!("jwt={}", token2))
        .add_header("Accept", "application/json")
        .json(&json!({ "user_ids": [] }))
        .await;

    create_response.assert_status_failure();

    assert!(create_response.text().contains("no users present"))
}

#[tokio::test]
#[serial]
async fn add_chat_users() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, _token2) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user1.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", token1))
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: false,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Discussion>();

    let private_discussion_user_ids = result.private_discussion_user_ids.unwrap();
    assert_eq!(private_discussion_user_ids.len(), 1);
    assert!(private_discussion_user_ids.contains(&user1.id.as_ref().unwrap().clone()));

    let create_response = server
        .post(&format!(
            "/api/discussions/{}/chat_users",
            result.id.as_ref().unwrap().to_raw().replace(":", "%3A")
        ))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .json(&json!({ "user_ids": [user2.id.as_ref().unwrap().to_raw()] }))
        .await;

    create_response.assert_status_ok();

    let create_response: axum_test::TestResponse = server
        .get("/api/discussions")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<Discussion>>();
    assert_eq!(
        result[0]
            .private_discussion_user_ids
            .as_ref()
            .unwrap()
            .len(),
        2
    );
    assert!(result[0]
        .private_discussion_user_ids
        .as_ref()
        .unwrap()
        .contains(&user2.id.as_ref().unwrap()),);
}

#[tokio::test]
#[serial]
async fn try_add_chat_users_by_not_owner() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user1.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", token1))
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user2.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Discussion>();

    let private_discussion_user_ids = result.private_discussion_user_ids.unwrap();
    assert_eq!(private_discussion_user_ids.len(), 2);
    assert!(private_discussion_user_ids.contains(&user1.id.as_ref().unwrap().clone()));
    assert!(private_discussion_user_ids.contains(&user2.id.as_ref().unwrap().clone()));

    let create_response = server
        .post(&format!(
            "/api/discussions/{}/chat_users",
            result.id.as_ref().unwrap().to_raw().replace(":", "%3A")
        ))
        .add_header("Cookie", format!("jwt={}", token2))
        .add_header("Accept", "application/json")
        .json(&json!({ "user_ids": [] }))
        .await;

    create_response.assert_status_failure();

    assert!(create_response.text().contains("no users present"));
}

#[tokio::test]
#[serial]
async fn try_remove_chat_users_into_read_only() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, _token2) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user1.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", token1))
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user2.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Discussion>();

    let private_discussion_user_ids = result.private_discussion_user_ids.unwrap();
    assert_eq!(private_discussion_user_ids.len(), 2);
    assert!(private_discussion_user_ids.contains(&user1.id.as_ref().unwrap().clone()));
    assert!(private_discussion_user_ids.contains(&user2.id.as_ref().unwrap().clone()));

    let create_response = server
        .delete(&format!(
            "/api/discussions/{}/chat_users",
            result.id.as_ref().unwrap().to_raw().replace(":", "%3A")
        ))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .json(&json!({ "user_ids": [user2.id.as_ref().unwrap().to_raw()] }))
        .await;

    create_response.assert_status_failure();

    assert!(create_response.text().contains("Forbidden"));
}

#[tokio::test]
#[serial]
async fn remove_chat_users() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, _token2) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user1.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", token1))
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user2.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: false,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Discussion>();

    let private_discussion_user_ids = result.private_discussion_user_ids.unwrap();
    assert_eq!(private_discussion_user_ids.len(), 2);
    assert!(private_discussion_user_ids.contains(&user1.id.as_ref().unwrap().clone()));
    assert!(private_discussion_user_ids.contains(&user2.id.as_ref().unwrap().clone()));

    let create_response = server
        .delete(&format!(
            "/api/discussions/{}/chat_users",
            result.id.as_ref().unwrap().to_raw().replace(":", "%3A")
        ))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .json(&json!({ "user_ids": [user2.id.as_ref().unwrap().to_raw()] }))
        .await;

    create_response.assert_status_ok();

    let create_response: axum_test::TestResponse = server
        .get("/api/discussions")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<Discussion>>();
    assert_eq!(
        result[0]
            .private_discussion_user_ids
            .as_ref()
            .unwrap()
            .len(),
        1
    );
    assert!(result[0]
        .private_discussion_user_ids
        .as_ref()
        .unwrap()
        .contains(&user1.id.as_ref().unwrap()),);
}

#[tokio::test]
#[serial]
async fn try_remove_owner_from_chat_users() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, _, _, _token2) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user1.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", token1))
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: false,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Discussion>();

    let private_discussion_user_ids = result.private_discussion_user_ids.unwrap();
    assert_eq!(private_discussion_user_ids.len(), 1);
    assert!(private_discussion_user_ids.contains(&user1.id.as_ref().unwrap().clone()));

    let create_response = server
        .delete(&format!(
            "/api/discussions/{}/chat_users",
            result.id.as_ref().unwrap().to_raw().replace(":", "%3A")
        ))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .json(&json!({ "user_ids": [user1.id.as_ref().unwrap().to_raw()] }))
        .await;

    create_response.assert_status_failure();

    assert!(create_response
        .text()
        .contains("Owner of the discussion can not remove yourself"));

    let create_response: axum_test::TestResponse = server
        .get("/api/discussions")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<Discussion>>();
    assert_eq!(
        result[0]
            .private_discussion_user_ids
            .as_ref()
            .unwrap()
            .len(),
        1
    );
    assert!(result[0]
        .private_discussion_user_ids
        .as_ref()
        .unwrap()
        .contains(&user1.id.as_ref().unwrap()),);
}

#[tokio::test]
#[serial]
async fn try_remove_chat_users_by_not_owner() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user1.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", token1))
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user2.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Discussion>();

    let private_discussion_user_ids = result.private_discussion_user_ids.unwrap();
    assert_eq!(private_discussion_user_ids.len(), 2);
    assert!(private_discussion_user_ids.contains(&user1.id.as_ref().unwrap().clone()));
    assert!(private_discussion_user_ids.contains(&user2.id.as_ref().unwrap().clone()));

    let create_response = server
        .post(&format!(
            "/api/discussions/{}/chat_users",
            result.id.as_ref().unwrap().to_raw().replace(":", "%3A")
        ))
        .add_header("Cookie", format!("jwt={}", token2))
        .add_header("Accept", "application/json")
        .json(&json!({ "user_ids": [] }))
        .await;

    create_response.assert_status_failure();
    assert!(create_response.text().contains("no users present"));
}

#[tokio::test]
#[serial]
async fn try_update_by_not_owner() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user1.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", token1))
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user2.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: false,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Discussion>();

    let private_discussion_user_ids = result.private_discussion_user_ids.unwrap();
    assert_eq!(private_discussion_user_ids.len(), 2);
    assert!(private_discussion_user_ids.contains(&user1.id.as_ref().unwrap().clone()));
    assert!(private_discussion_user_ids.contains(&user2.id.as_ref().unwrap().clone()));

    let create_response = server
        .patch(&format!(
            "/api/discussions/{}",
            result.id.as_ref().unwrap().to_raw().replace(":", "%3A")
        ))
        .add_header("Cookie", format!("jwt={}", token2))
        .add_header("Accept", "application/json")
        .json(&json!({ "title": "Hello "}))
        .await;

    create_response.assert_status_failure();
    assert!(create_response.text().contains("not authorized"));
}

#[tokio::test]
#[serial]
async fn update() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, _token2) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user1.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", token1))
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user2.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: false,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Discussion>();

    let private_discussion_user_ids = result.private_discussion_user_ids.unwrap();
    assert_eq!(private_discussion_user_ids.len(), 2);
    assert!(private_discussion_user_ids.contains(&user1.id.as_ref().unwrap().clone()));
    assert!(private_discussion_user_ids.contains(&user2.id.as_ref().unwrap().clone()));

    let disc_id = result.id.as_ref().unwrap();
    let create_response = server
        .patch(&format!(
            "/api/discussions/{}",
            disc_id.to_raw().replace(":", "%3A")
        ))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .json(&json!({ "title": "Hello"}))
        .await;

    create_response.assert_status_ok();
    let create_response: axum_test::TestResponse = server
        .get("/api/discussions")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<Discussion>>();
    assert_eq!(result.len(), 1);
    let id = Some(disc_id.clone());
    let disc = result.into_iter().find(|item| item.id == id).unwrap();

    assert_eq!(disc.title, Some("Hello".to_string()));
}

#[tokio::test]
#[serial]
async fn delete_read_only() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, _token2) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user1.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", token1))
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user2.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Discussion>();

    let private_discussion_user_ids = result.private_discussion_user_ids.unwrap();
    assert_eq!(private_discussion_user_ids.len(), 2);
    assert!(private_discussion_user_ids.contains(&user1.id.as_ref().unwrap().clone()));
    assert!(private_discussion_user_ids.contains(&user2.id.as_ref().unwrap().clone()));

    let disc_id = result.id.as_ref().unwrap();
    let create_response = server
        .delete(&format!(
            "/api/discussions/{}",
            disc_id.to_raw().replace(":", "%3A")
        ))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_ok();

    let create_response: axum_test::TestResponse = server
        .get("/api/discussions")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<Discussion>>();
    assert_eq!(result.len(), 0);
}

#[tokio::test]
#[serial]
async fn try_delete_by_not_owner() {
    let (server, _) = create_test_server().await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user1.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", token1))
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user2.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Discussion>();

    let private_discussion_user_ids = result.private_discussion_user_ids.unwrap();
    assert_eq!(private_discussion_user_ids.len(), 2);
    assert!(private_discussion_user_ids.contains(&user1.id.as_ref().unwrap().clone()));
    assert!(private_discussion_user_ids.contains(&user2.id.as_ref().unwrap().clone()));

    let disc_id = result.id.as_ref().unwrap();
    let create_response = server
        .delete(&format!(
            "/api/discussions/{}",
            disc_id.to_raw().replace(":", "%3A")
        ))
        .add_header("Cookie", format!("jwt={}", token2))
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_failure();
    assert!(create_response.text().contains("not authorized"));
    let create_response: axum_test::TestResponse = server
        .get("/api/discussions")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<Discussion>>();
    assert_eq!(result.len(), 1);
}
