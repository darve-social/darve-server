mod helpers;

use crate::helpers::create_fake_login_test_user;
use crate::helpers::post_helpers::create_fake_post;
use darve_server::entities::community::community_entity::CommunityDbService;
use darve_server::entities::community::discussion_entity::{Discussion, DiscussionDbService};
use darve_server::entities::task::task_request_entity::TaskRequest;
use darve_server::services::discussion_service::CreateDiscussion;
use fake::{faker, Fake};
use serde_json::json;

test_with_server!(delete_post_test, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;

    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

    let post = create_fake_post(server, &default_discussion, None, None).await;

    let response = server
        .delete(&format!("/api/posts/{}", post.id))
        .add_header("Cookie", format!("jwt={}", token))
        .await;

    response.assert_status_ok();
    let post_response = server
        .get(&format!("/api/posts/{}", post.id))
        .add_header("Cookie", format!("jwt={}", token))
        .await;

    post_response.assert_status_not_found();
});

test_with_server!(
    try_to_delete_post_by_non_discussion_owner,
    |server, ctx_state, config| {
        let (server, user1, _, _) = create_fake_login_test_user(&server).await;
        let default_discussion =
            DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());
        let post = create_fake_post(server, &default_discussion, None, None).await;
        let (server, _, _, token2) = create_fake_login_test_user(&server).await;

        let response = server
            .delete(&format!("/api/posts/{}", post.id))
            .add_header("Cookie", format!("jwt={}", token2))
            .await;

        response.assert_status_forbidden();
    }
);

test_with_server!(
    try_to_delete_post_with_invalid_id,
    |server, ctx_state, config| {
        let (server, _, _, token) = create_fake_login_test_user(&server).await;

        let response = server
            .delete("/api/posts/post:not_a_real_id")
            .add_header("Cookie", format!("jwt={}", token))
            .await;

        response.assert_status_not_found();
    }
);

test_with_server!(
    try_to_delete_post_by_not_discussion_owner,
    |server, ctx_state, config| {
        let (server, _, _, user_token) = create_fake_login_test_user(&server).await;
        let (server, user, _, _) = create_fake_login_test_user(&server).await;

        let default_discussion =
            DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

        let post = create_fake_post(server, &default_discussion, None, None).await;

        server
            .delete(&format!("/api/posts/{}", post.id))
            .add_header("Cookie", format!("jwt={}", user_token))
            .await
            .assert_status_forbidden();
    }
);

test_with_server!(
    try_to_delete_post_by_with_task,
    |server, ctx_state, config| {
        let (server, user1, _, user1_token) = create_fake_login_test_user(&server).await;
        server
            .get(&format!("/test/api/deposit/{}/10000", user1.username,))
            .add_header("Accept", "application/json")
            .json("")
            .await;

        let (server, user, _, _) = create_fake_login_test_user(&server).await;

        let default_discussion =
            DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

        let post = create_fake_post(server, &default_discussion, None, None).await;

        server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                "participants": vec![user.id.as_ref().unwrap().to_raw()],
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", user1_token))
            .add_header("Accept", "application/json")
            .await
            .assert_status_ok();

        server
            .delete(&format!("/api/posts/{}", post.id))
            .add_header("Cookie", format!("jwt={}", user1_token))
            .await
            .assert_status_forbidden();
    }
);
test_with_server!(try_to_delete_delivery_post, |server, ctx_state, config| {
    let (server, user1, _, user1_token) = create_fake_login_test_user(&server).await;
    let user1_discussion =
        DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());
    let delivery_post = create_fake_post(server, &user1_discussion, None, None).await;

    let (server, user, _, user_token) = create_fake_login_test_user(&server).await;
    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

    server
        .get(&format!("/test/api/deposit/{}/10000", user.username))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    let post = create_fake_post(server, &default_discussion, None, None).await;

    let task_response = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user1.id.as_ref().unwrap().to_raw()],
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", user_token))
        .add_header("Accept", "application/json")
        .await;
    let task_id = task_response.json::<TaskRequest>().id.unwrap().to_raw();

    server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", user1_token))
        .add_header("Accept", "application/json")
        .await
        .assert_status_success();
    server
        .post(&format!("/api/tasks/{}/deliver", task_id))
        .json(&json!({"post_id": delivery_post.id}))
        .add_header("Cookie", format!("jwt={}", user1_token))
        .add_header("Accept", "application/json")
        .await
        .assert_status_success();
    server
        .delete(&format!("/api/posts/{}", post.id))
        .add_header("Cookie", format!("jwt={}", user_token))
        .await
        .assert_status_forbidden();
    server
        .delete(&format!("/api/posts/{}", delivery_post.id))
        .add_header("Cookie", format!("jwt={}", user1_token))
        .await
        .assert_status_forbidden();
});

test_with_server!(
    try_to_delete_post_in_private_discussion,
    |server, ctx_state, config| {
        let (server, user, _, user_token) = create_fake_login_test_user(&server).await;
        let (server, user1, _, user1_token) = create_fake_login_test_user(&server).await;

        let comm_id = CommunityDbService::get_profile_community_id(user1.id.as_ref().unwrap());
        let create_response = server
            .post("/api/discussions")
            .json(&CreateDiscussion {
                community_id: comm_id.to_raw(),
                title: "The Discussion".to_string(),
                image_uri: None,
                chat_user_ids: vec![user.id.as_ref().unwrap().to_raw()].into(),
                private_discussion_users_final: true,
            })
            .add_header("Accept", "application/json")
            .await;
        create_response.assert_status_ok();
        let disc_id = create_response.json::<Discussion>().id;
        let post = create_fake_post(server, &disc_id, None, None).await;
        server
            .delete(&format!("/api/posts/{}", post.id))
            .add_header("Cookie", format!("jwt={}", user1_token))
            .await
            .assert_status_forbidden();

        server
            .delete(&format!("/api/posts/{}", post.id))
            .add_header("Cookie", format!("jwt={}", user_token))
            .await
            .assert_status_forbidden();
    }
);
