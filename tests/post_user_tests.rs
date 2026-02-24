mod helpers;
use crate::helpers::RecordIdExt;

use crate::helpers::{create_fake_login_test_user, post_helpers::create_fake_post};
use darve_server::{
    entities::community::{
        community_entity::CommunityDbService,
        discussion_entity::{Discussion, DiscussionDbService},
        post_entity::PostUserStatus,
    },
    interfaces::repositories::post_user::PostUserRepositoryInterface,
    middleware::utils::string_utils::get_str_thing,
    models::view::discussion_user::DiscussionUserView,
    services::discussion_service::CreateDiscussion,
};

test_with_server!(
    test_post_mark_as_deliver_success,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let discussion_id =
            DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());
        let fake_post = create_fake_post(&server, &discussion_id, None, None).await;

        let response = server
            .post(&format!("/api/posts/{}/deliver", fake_post.id))
            .await;

        response.assert_status_success();

        let status = ctx_state
            .db
            .post_users
            .get(
                user.id.as_ref().unwrap().clone(),
                get_str_thing(&fake_post.id).unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(status, Some(PostUserStatus::Delivered));
    }
);

test_with_server!(
    test_post_mark_as_deliver_post_not_found,
    |server, ctx_state, config| {
        let (_, _, _, _) = create_fake_login_test_user(&server).await;
        let response = server.post("/api/posts/post:nonexistent/deliver").await;
        response.assert_status_not_found();
    }
);

test_with_server!(
    test_post_mark_as_read_success,
    |server, ctx_state, config| {
        // Create a test user
        let (_, user, _password, _) = create_fake_login_test_user(&server).await;

        let discussion_id =
            DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());
        let fake_post = create_fake_post(&server, &discussion_id, None, None).await;

        // First, create a post_user relation by calling deliver
        let deliver_response = server
            .post(&format!("/api/posts/{}/deliver", fake_post.id))
            .await;
        deliver_response.assert_status_success();

        // Now call the read endpoint
        let response = server
            .post(&format!("/api/posts/{}/read", fake_post.id))
            .await;

        response.assert_status_success();

        let status = ctx_state
            .db
            .post_users
            .get(
                user.id.as_ref().unwrap().clone(),
                get_str_thing(&fake_post.id).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status, Some(PostUserStatus::Seen));
    }
);

test_with_server!(
    test_post_mark_as_read_without_prior_deliver,
    |server, ctx_state, config| {
        let (_, user, _password, _token) = create_fake_login_test_user(&server).await;
        let discussion_id =
            DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap());
        let fake_post = create_fake_post(&server, &discussion_id, None, None).await;

        let response = server
            .post(&format!("/api/posts/{}/read", fake_post.id))
            .await;

        response.assert_status_success();

        let status = ctx_state
            .db
            .post_users
            .get(
                user.id.as_ref().unwrap().clone(),
                get_str_thing(&fake_post.id).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status, Some(PostUserStatus::Seen));
    }
);

test_with_server!(
    test_post_mark_as_read_post_not_found,
    |server, ctx_state, config| {
        let (_, _, _, _) = create_fake_login_test_user(&server).await;
        let response = server.post("/api/posts/post:nonexistent/read").await;

        response.assert_status_not_found();
    }
);

test_with_server!(
    test_post_mark_as_read_invalid_post_id,
    |server, ctx_state, config| {
        let (_, _, _, _) = create_fake_login_test_user(&server).await;
        let response = server.post("/api/posts/invalid_id_format/read").await;
        response.assert_status_failure();
    }
);

test_with_server!(
    test_post_deliver_and_read_sequence,
    |server, ctx_state, config| {
        // Create a test user
        let (_, user, _password, _) = create_fake_login_test_user(&server).await;

        let discussion_id =
            DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap());
        let fake_post = create_fake_post(&server, &discussion_id, None, None).await;

        // 1. First mark as delivered
        let deliver_response = server
            .post(&format!("/api/posts/{}/deliver", fake_post.id))
            .await;
        deliver_response.assert_status_success();

        // 2. Then mark as read
        let read_response = server
            .post(&format!("/api/posts/{}/read", fake_post.id))
            .await;
        read_response.assert_status_success();

        let status = ctx_state
            .db
            .post_users
            .get(
                user.id.as_ref().unwrap().clone(),
                get_str_thing(&fake_post.id).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status, Some(PostUserStatus::Seen));
    }
);

test_with_server!(
    try_to_deliver_post_after_marking_as_read,
    |server, ctx_state, config| {
        // Create a test user
        let (_, user0, _password, _token0) = create_fake_login_test_user(&server).await;
        let (_, user, _password, token) = create_fake_login_test_user(&server).await;

        let comm_id = CommunityDbService::get_profile_community_id(user.id.as_ref().unwrap());

        let create_response = server
            .post("/api/discussions")
            .add_header("Cookie", format!("jwt={}", token))
            .json(&CreateDiscussion {
                community_id: comm_id.to_raw(),
                title: "The Discussion".to_string(),
                image_uri: None,
                chat_user_ids: vec![user0.id.as_ref().unwrap().to_raw()].into(),
                private_discussion_users_final: false,
            })
            .add_header("Accept", "application/json")
            .await;

        let discussion_id = create_response.json::<Discussion>().id;

        let fake_post = create_fake_post(&server, &discussion_id, None, None).await;

        // 1. First mark as delivered
        let deliver_response = server
            .post(&format!("/api/posts/{}/deliver", fake_post.id))
            .await;
        deliver_response.assert_status_success();

        // 2. Then mark as read
        let read_response = server
            .post(&format!("/api/posts/{}/read", fake_post.id))
            .await;
        read_response.assert_status_success();

        let status = ctx_state
            .db
            .post_users
            .get(
                user.id.as_ref().unwrap().clone(),
                get_str_thing(&fake_post.id).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status, Some(PostUserStatus::Seen));

        let deliver_response = server
            .post(&format!("/api/posts/{}/deliver", fake_post.id))
            .await;
        deliver_response.assert_status_forbidden();

        let latest_posts = server
            .get("/api/users/current/latest_posts")
            .add_header("Cookie", format!("jwt={}", token))
            .await
            .json::<Vec<DiscussionUserView>>();
        let users_status = latest_posts
            .first()
            .as_ref()
            .unwrap()
            .latest_post
            .as_ref()
            .unwrap()
            .users_status
            .as_ref()
            .unwrap();

        assert_eq!(users_status.len(), 1);
        assert_eq!(users_status[0].status, PostUserStatus::Seen);
    }
);
