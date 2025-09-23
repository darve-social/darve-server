mod helpers;

use axum_test::multipart::MultipartForm;
use darve_server::access::base::role::Role;
use darve_server::entities::community::{community_entity, discussion_entity};
use darve_server::middleware;
use darve_server::models::view::discussion::DiscussionView;
use darve_server::services::discussion_service::CreateDiscussion;
use serde_json::json;

use community_entity::CommunityDbService;
use discussion_entity::{Discussion, DiscussionDbService};
use middleware::ctx::Ctx;
use middleware::utils::db_utils::IdentIdName;

use crate::helpers::create_fake_login_test_user;

test_with_server!(get_discussion_view, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;

    let comm_id = CommunityDbService::get_profile_community_id(&user.id.as_ref().unwrap());

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

    let disc_id = created.id;
    create_response.assert_status_success();

    let post_name = "post title Name 1".to_string();
    let create_post = server
        .post(format!("/api/discussions/{disc_id}/posts").as_str())
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "contentttt111"),
        )
        .add_header("Accept", "application/json")
        .await;
    create_post.assert_status_success();

    let post_name2 = "post title Name 2?&$^%! <>end".to_string();
    let create_response2 = server
        .post(format!("/api/discussions/{disc_id}/posts").as_str())
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name2.clone())
                .add_text("content", "contentttt222"),
        )
        .add_header("Accept", "application/json")
        .await;
    create_response2.assert_status_success();
});

test_with_server!(create_discussion, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;

    let comm_id = CommunityDbService::get_profile_community_id(&user.id.as_ref().unwrap());

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

    let ctx = &Ctx::new(Ok("user_ident".parse().unwrap()), false);

    let disc_db = DiscussionDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };

    let discussion = disc_db
        .get(IdentIdName::Id(created.id.clone()).into())
        .await
        .unwrap();

    assert_eq!(discussion.belongs_to.eq(&comm_id.clone()), true);
});

test_with_server!(create_chat_discussion, |server, ctx_state, config| {
    let (server, user1, _, _) = create_fake_login_test_user(&server).await;

    let (server, user2, _, _) = create_fake_login_test_user(&server).await;

    let comm_id = CommunityDbService::get_profile_community_id(user2.id.as_ref().unwrap());
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
    let create_response = server
        .get("/api/discussions")
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_ok();
    let result = create_response.json::<Vec<DiscussionView>>();

    let disc = result.first().unwrap();

    assert_eq!(disc.users.len(), 2);
    let owner = disc
        .users
        .iter()
        .find(|u| &u.user.id == user2.id.as_ref().unwrap());

    assert!(owner.is_some());
    assert_eq!(owner.unwrap().role, Role::Editor.to_string());
    let member = disc
        .users
        .iter()
        .find(|u| &u.user.id == user1.id.as_ref().unwrap());

    assert!(member.is_some());
    assert_eq!(member.unwrap().role, Role::Member.to_string());
});

test_with_server!(
    try_to_create_the_same_read_only,
    |server, ctx_state, config| {
        let (server, user1, _, _) = create_fake_login_test_user(&server).await;

        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let comm_id =
            CommunityDbService::get_profile_community_id(&user1.id.as_ref().unwrap().clone());
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

        create_response.assert_status_failure();

        let comm_id =
            CommunityDbService::get_profile_community_id(&user2.id.as_ref().unwrap().clone());
        let create_response = server
            .post("/api/discussions")
            .json(&CreateDiscussion {
                community_id: comm_id.clone().to_raw(),
                title: "The New Discussion".to_string(),
                image_uri: None,
                chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
                private_discussion_users_final: true,
            })
            .add_header("Accept", "application/json")
            .await;

        create_response.assert_status_ok();
        let result = create_response.json::<Discussion>();

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
);

test_with_server!(
    try_to_create_the_same_not_read_only,
    |server, ctx_state, config| {
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
);

test_with_server!(get_discussions, |server, ctx_state, config| {
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;
    let (server, user3, _, token3) = create_fake_login_test_user(&server).await;

    let comm_id = format!("community:{}", user2.id.as_ref().unwrap().id.to_string());
    let create_response = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", token2))
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
        .add_header("Cookie", format!("jwt={}", token2))
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![
                user1.id.as_ref().unwrap().to_raw(),
                user3.id.as_ref().unwrap().to_raw(),
            ]
            .into(),
            private_discussion_users_final: false,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();
    let create_response = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", token2))
        .json(&CreateDiscussion {
            community_id: comm_id.clone(),
            title: "The Discussion 1".to_string(),
            image_uri: None,
            chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_ok();

    let create_response = server
        .get("/api/discussions?type=Private")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token2))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<DiscussionView>>();
    assert_eq!(result.len(), 2);

    let create_response = server
        .get("/api/discussions?type=Private")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<DiscussionView>>();
    assert_eq!(result.len(), 2);

    let create_response = server
        .get("/api/discussions?type=Private")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token3))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<DiscussionView>>();
    assert_eq!(result.len(), 1);
});

test_with_server!(
    try_add_chat_users_into_read_only,
    |server, ctx_state, config| {
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

        let create_response = server
            .post(&format!(
                "/api/discussions/{}/chat_users",
                result.id.to_raw().replace(":", "%3A")
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
                result.id.to_raw().replace(":", "%3A")
            ))
            .add_header("Cookie", format!("jwt={}", token2))
            .add_header("Accept", "application/json")
            .json(&json!({ "user_ids": [] }))
            .await;

        create_response.assert_status_failure();

        assert!(create_response.text().contains("no users present"))
    }
);

test_with_server!(add_chat_users, |server, ctx_state, config| {
    let (server, user, _, _token) = create_fake_login_test_user(&server).await;
    let (server, user0, _, _token0) = create_fake_login_test_user(&server).await;
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

    let create_response = server
        .post(&format!(
            "/api/discussions/{}/chat_users",
            result.id.to_raw()
        ))
        .add_header("Cookie", format!("jwt={}", token2))
        .add_header("Accept", "application/json")
        .json(&json!({ "user_ids": [user.id.as_ref().unwrap().to_raw()] }))
        .await;

    create_response.assert_status_forbidden();

    let create_response = server
        .post(&format!(
            "/api/discussions/{}/chat_users",
            result.id.to_raw()
        ))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .json(&json!({ "user_ids": [user0.id.as_ref().unwrap().to_raw()] }))
        .await;

    create_response.assert_status_ok();

    let create_response: axum_test::TestResponse = server
        .get("/api/discussions?type=Private")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<DiscussionView>>();
    assert_eq!(result[0].users.len(), 3);
    assert!(result[0]
        .users
        .iter()
        .find(|u| &u.user.id == user1.id.as_ref().unwrap())
        .is_some());
    assert!(result[0]
        .users
        .iter()
        .find(|u| &u.user.id == user.id.as_ref().unwrap())
        .is_none());
});

test_with_server!(
    try_add_chat_users_by_not_owner,
    |server, ctx_state, config| {
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

        let create_response = server
            .post(&format!(
                "/api/discussions/{}/chat_users",
                result.id.to_raw().replace(":", "%3A")
            ))
            .add_header("Cookie", format!("jwt={}", token2))
            .add_header("Accept", "application/json")
            .json(&json!({ "user_ids": [] }))
            .await;

        create_response.assert_status_failure();

        assert!(create_response.text().contains("no users present"));
    }
);

test_with_server!(
    try_remove_chat_users_into_read_only,
    |server, ctx_state, config| {
        let (server, user0, _, _) = create_fake_login_test_user(&server).await;
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

        let create_response = server
            .delete(&format!(
                "/api/discussions/{}/chat_users",
                result.id.to_raw().replace(":", "%3A")
            ))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .json(&json!({ "user_ids": [user2.id.as_ref().unwrap().to_raw()] }))
            .await;

        create_response.assert_status_forbidden();

        let create_response = server
            .delete(&format!(
                "/api/discussions/{}/chat_users",
                result.id.to_raw().replace(":", "%3A")
            ))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .json(&json!({ "user_ids": [user0.id.as_ref().unwrap().to_raw()] }))
            .await;

        create_response.assert_status_forbidden();
    }
);

test_with_server!(remove_chat_users, |server, ctx_state, config| {
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

    let create_response = server
        .delete(&format!(
            "/api/discussions/{}/chat_users",
            result.id.to_raw().replace(":", "%3A")
        ))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .json(&json!({ "user_ids": [user2.id.as_ref().unwrap().to_raw()] }))
        .await;

    create_response.assert_status_ok();

    let create_response: axum_test::TestResponse = server
        .get("/api/discussions?type=Private")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<DiscussionView>>();
    assert_eq!(result[0].users.len(), 1);
    assert!(result[0]
        .users
        .iter()
        .find(|u| &u.user.id == user1.id.as_ref().unwrap())
        .is_some());
});

test_with_server!(
    try_remove_owner_from_chat_users,
    |server, ctx_state, config| {
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

        let create_response = server
            .delete(&format!(
                "/api/discussions/{}/chat_users",
                result.id.to_raw().replace(":", "%3A")
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
            .get("/api/discussions?type=Private")
            .add_header("Accept", "application/json")
            .add_header("Cookie", format!("jwt={}", token1))
            .await;

        create_response.assert_status_ok();
        let result = create_response.json::<Vec<DiscussionView>>();
        assert_eq!(result[0].users.len(), 2);
        assert!(result[0]
            .users
            .iter()
            .find(|u| &u.user.id == user1.id.as_ref().unwrap())
            .is_some());
    }
);

test_with_server!(
    try_remove_chat_users_by_not_owner,
    |server, ctx_state, config| {
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

        let create_response = server
            .post(&format!(
                "/api/discussions/{}/chat_users",
                result.id.to_raw().replace(":", "%3A")
            ))
            .add_header("Cookie", format!("jwt={}", token2))
            .add_header("Accept", "application/json")
            .json(&json!({ "user_ids": [] }))
            .await;

        create_response.assert_status_failure();
        assert!(create_response.text().contains("no users present"));
    }
);

test_with_server!(try_update_by_not_owner, |server, ctx_state, config| {
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

    let create_response = server
        .patch(&format!(
            "/api/discussions/{}",
            result.id.to_raw().replace(":", "%3A")
        ))
        .add_header("Cookie", format!("jwt={}", token2))
        .add_header("Accept", "application/json")
        .json(&json!({ "title": "Hello "}))
        .await;

    create_response.assert_status_forbidden();
});

test_with_server!(update, |server, ctx_state, config| {
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

    let disc_id = result.id;
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
        .get("/api/discussions?type=Private")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<DiscussionView>>();
    assert_eq!(result.len(), 1);
    let id = disc_id.clone();
    let disc = result.into_iter().find(|item| item.id == id).unwrap();

    assert_eq!(disc.title, Some("Hello".to_string()));
});

test_with_server!(update_alias, |server, ctx_state, config| {
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

    let disc_id = result.id;
    let create_response = server
        .post(&format!("/api/discussions/{}/alias", disc_id.to_raw()))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .json(&json!({ "alias": "Hello"}))
        .await;

    create_response.assert_status_ok();
    let create_response: axum_test::TestResponse = server
        .get("/api/discussions?type=Private")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<DiscussionView>>();
    assert_eq!(result.len(), 1);
    let id = disc_id.clone();
    let disc = result.into_iter().find(|item| item.id == id).unwrap();

    assert_eq!(disc.alias, Some("Hello".to_string()));
    let create_response: axum_test::TestResponse = server
        .get("/api/discussions?type=Private")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token2))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<DiscussionView>>();
    assert_eq!(result.len(), 1);
    let id = disc_id.clone();
    let disc = result.into_iter().find(|item| item.id == id).unwrap();

    assert_eq!(disc.alias, None);
});

test_with_server!(unset_update_alias, |server, ctx_state, config| {
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

    let disc_id = result.id;
    let create_response = server
        .post(&format!("/api/discussions/{}/alias", disc_id.to_raw()))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .json(&json!({ "alias": "Hello"}))
        .await;

    create_response.assert_status_ok();
    let create_response: axum_test::TestResponse = server
        .get("/api/discussions?type=Private")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<DiscussionView>>();
    assert_eq!(result.len(), 1);
    let id = disc_id.clone();
    let disc = result.into_iter().find(|item| item.id == id).unwrap();

    assert_eq!(disc.alias, Some("Hello".to_string()));
    let create_response = server
        .post(&format!("/api/discussions/{}/alias", disc_id.to_raw()))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .json(&json!({ "alias": null}))
        .await;
    create_response.assert_status_ok();

    let create_response: axum_test::TestResponse = server
        .get("/api/discussions?type=Private")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    create_response.assert_status_ok();
    let result = create_response.json::<Vec<DiscussionView>>();
    assert_eq!(result.len(), 1);
    let id = disc_id.clone();
    let disc = result.into_iter().find(|item| item.id == id).unwrap();

    assert_eq!(disc.alias, None);
});

test_with_server!(
    try_to_update_alias_for_public_disc,
    |server, ctx_state, config| {
        let (server, user2, _, token2) = create_fake_login_test_user(&server).await;

        let disc_id = format!("discussion:{}", user2.id.as_ref().unwrap().id.to_string());
        let create_response = server
            .post(&format!("/api/discussions/{}/alias", disc_id))
            .add_header("Cookie", format!("jwt={}", token2))
            .add_header("Accept", "application/json")
            .json(&json!({ "alias": "Hello"}))
            .await;

        create_response.assert_status_forbidden();
    }
);

test_with_server!(
    try_to_update_alias_by_non_member,
    |server, ctx_state, config| {
        let (server, _user0, _, token0) = create_fake_login_test_user(&server).await;
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

        let disc_id = result.id;
        let create_response = server
            .post(&format!("/api/discussions/{}/alias", disc_id.to_raw()))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .json(&json!({ "alias": "Hello"}))
            .await;

        create_response.assert_status_forbidden();
    }
);
