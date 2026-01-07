mod helpers;

use crate::helpers::{create_fake_login_test_user, task_helpers};
use darve_server::{
    entities::{
        community::discussion_entity::DiscussionDbService, task_request::TaskRequestEntity,
    },
    middleware::utils::string_utils::get_string_thing,
    models::view::post::PostView,
    routes::posts::PostLikeResponse,
};
use fake::{faker, Fake};
use helpers::post_helpers::{self, create_fake_post};
use serde_json::json;

test_with_server!(create_post_like, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();
    let result =
        DiscussionDbService::get_profile_discussion_id(&get_string_thing(user_ident).unwrap());

    let result = create_fake_post(server, &result, None, None).await;

    // check like and number
    let response = post_helpers::create_post_like(&server, &result.id, None).await;
    response.assert_status_ok();
    let likes_nr = response.json::<PostLikeResponse>().likes_count;
    assert_eq!(likes_nr, 1);

    // check delete and count
    let response = post_helpers::delete_post_like(&server, &result.id).await;
    response.assert_status_ok();
    let likes_nr = response.json::<PostLikeResponse>().likes_count;
    assert_eq!(likes_nr, 0);

    // throw if like post that does not exist
    let response = post_helpers::create_post_like(&server, "post:that_does_not_exist", None).await;
    response.assert_status_failure();
});

test_with_server!(create_post_like_with_count, |server, ctx_state, config| {
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    // let result = DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());
    // let deliver = create_fake_post(server, &result, None, None).await;

    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let result = DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

    server
        .get(&format!("/test/api/deposit/{}/{}", user.username, 1000))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await
        .assert_status_ok();
    let post = create_fake_post(server, &result, None, None).await;

    let task = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(1000),
            "participants": vec![user1.id.as_ref().unwrap().to_raw()],
            "content": faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await
        .json::<TaskRequestEntity>();

    server
        .post(&format!("/api/tasks/{}/accept", task.id.to_raw()))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await
        .assert_status_success();

    task_helpers::success_deliver_task(&server, &task.id, &token1)
        .await
        .unwrap();

    let response = server
        .post(format!("/api/posts/{}/like", post.id).as_str())
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token))
        .json(&json!({ "count": 4 }))
        .await;

    response.assert_status_ok();
    let likes_nr = response.json::<PostLikeResponse>().likes_count;
    assert_eq!(likes_nr, 4);
});

test_with_server!(try_gives_100_likes, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let result = DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());
    let post = create_fake_post(server, &result, None, None).await;

    let response = server
        .post(format!("/api/posts/{}/like", post.id).as_str())
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token))
        .json(&json!({ "count": 100 }))
        .await;

    response.assert_status_failure();
});

test_with_server!(try_gives_1_likes, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let result = DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());
    let post = create_fake_post(server, &result, None, None).await;

    let response = server
        .post(format!("/api/posts/{}/like", post.id).as_str())
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token))
        .json(&json!({ "count": 1 }))
        .await;

    response.assert_status_failure();
});

test_with_server!(
    try_gives_likes_without_enough_credits,
    |server, ctx_state, config| {
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;

        let (server, user, _, token) = create_fake_login_test_user(&server).await;
        let result = DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

        server
            .get(&format!("/test/api/deposit/{}/{}", user.username, 1000))
            .add_header("Cookie", format!("jwt={}", token))
            .add_header("Accept", "application/json")
            .await
            .assert_status_ok();
        let post = create_fake_post(server, &result, None, None).await;

        let task = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(600),
                "participants": vec![user1.id.as_ref().unwrap().to_raw()],
                "content": faker::lorem::en::Sentence(7..20).fake::<String>(),
                "delivery_period": 1,
            }))
            .add_header("Cookie", format!("jwt={}", token))
            .add_header("Accept", "application/json")
            .await
            .json::<TaskRequestEntity>();

        server
            .post(&format!("/api/tasks/{}/accept", task.id.to_raw()))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;

        task_helpers::success_deliver_task(&server, &task.id, &token1)
            .await
            .unwrap();

        let response = server
            .post(format!("/api/posts/{}/like", post.id).as_str())
            .add_header("Accept", "application/json")
            .add_header("Cookie", format!("jwt={}", token))
            .json(&json!({ "count": 8 }))
            .await;

        response.assert_status_failure();
    }
);

test_with_server!(update_likes, |server, ctx_state, config| {
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;

    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let result = DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

    server
        .get(&format!("/test/api/deposit/{}/{}", user.username, 1000))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await
        .assert_status_ok();
    let post = create_fake_post(server, &result, None, None).await;

    let task = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(600),
            "participants": vec![user1.id.as_ref().unwrap().to_raw()],
            "content": faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await
        .json::<TaskRequestEntity>();

    server
        .post(&format!("/api/tasks/{}/accept", task.id.to_raw()))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await
        .assert_status_success();

    task_helpers::success_deliver_task(&server, &task.id, &token1)
        .await
        .unwrap();

    let response = server
        .post(format!("/api/posts/{}/like", post.id).as_str())
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token))
        .json(&json!({ "count": 4 }))
        .await;

    response.assert_status_ok();
    let likes_nr = response.json::<PostLikeResponse>().likes_count;
    assert_eq!(likes_nr, 4);

    let response = server
        .post(format!("/api/posts/{}/like", post.id).as_str())
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token))
        .json(&json!({ "count": 2 }))
        .await;

    response.assert_status_ok();
    let likes_nr = response.json::<PostLikeResponse>().likes_count;
    assert_eq!(likes_nr, 2);
});

test_with_server!(post_likes, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();
    let disc =
        DiscussionDbService::get_profile_discussion_id(&get_string_thing(user_ident).unwrap());
    let result = create_fake_post(server, &disc, None, None).await;

    let response = post_helpers::create_post_like(&server, &result.id, None).await;
    response.assert_status_ok();
    let likes_nr = response.json::<PostLikeResponse>().likes_count;
    assert_eq!(likes_nr, 1);

    let (server, user1, _, _) = create_fake_login_test_user(&server).await;
    let response = post_helpers::create_post_like(&server, &result.id, None).await;
    response.assert_status_ok();
    let likes_nr = response.json::<PostLikeResponse>().likes_count;

    assert_eq!(likes_nr, 2);

    let posts = server
        .get(format!("/api/discussions/{}/posts", disc).as_str())
        .await
        .json::<Vec<PostView>>();

    assert_eq!(posts.len(), 1);
    let post = posts.first().unwrap();
    let liked_by = post.liked_by.clone().unwrap_or_default();
    assert!(liked_by.contains(user1.id.as_ref().unwrap()));
    assert!(!liked_by.contains(user.id.as_ref().unwrap()));
});
