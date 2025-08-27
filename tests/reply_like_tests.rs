mod helpers;

use crate::helpers::create_fake_login_test_user;
use crate::helpers::post_helpers::create_fake_reply;
use darve_server::routes::reply::LikeResponse;
use darve_server::{
    entities::community::discussion_entity::DiscussionDbService, models::view::reply::ReplyView,
};
use fake::{faker, Fake};
use helpers::post_helpers::{self, create_fake_post};
use serde_json::json;

test_with_server!(create_reply_like, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let discussion = DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap());
    let post = create_fake_post(server, &discussion, None, None).await;
    let reply = create_fake_reply(server, &post.id).await;
    // check like and number
    let response = post_helpers::create_reply_like(&server, &reply.id.to_raw(), None).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 1);

    // check delete and count
    let response = post_helpers::delete_reply_like(&server, &reply.id.to_raw()).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 0);

    // throw if like post that does not exist
    let response = post_helpers::create_reply_like(&server, "post:that_does_not_exist", None).await;
    response.assert_status_failure();
});

test_with_server!(create_reply_like_with_count, |server, ctx_state, config| {
    let (server, user1, _, _) = create_fake_login_test_user(&server).await;
    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let discussion = DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap());
    let post = create_fake_post(server, &discussion, None, None).await;
    server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user.id.as_ref().unwrap().to_raw(),
            1000
        ))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await
        .assert_status_ok();

    server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participant": user1.id.as_ref().unwrap().to_raw(),
            "content": faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await
        .assert_status_success();
    let reply = create_fake_reply(server, &post.id).await;
    let response = post_helpers::create_reply_like(&server, &reply.id.to_raw(), Some(4)).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 4);
});

test_with_server!(update_likes, |server, ctx_state, config| {
    let (server, user1, _, _) = create_fake_login_test_user(&server).await;
    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let discussion = DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap());
    let post = create_fake_post(server, &discussion, None, None).await;
    let reply = create_fake_reply(server, &post.id).await;

    let response = post_helpers::create_reply_like(&server, &reply.id.to_raw(), None).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 1);

    let response = post_helpers::create_reply_like(&server, &reply.id.to_raw(), Some(10)).await;
    response.assert_status_forbidden();

    server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user.id.as_ref().unwrap().to_raw(),
            1000
        ))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await
        .assert_status_ok();

    server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participant": user1.id.as_ref().unwrap().to_raw(),
            "content": faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await
        .assert_status_success();

    let response = post_helpers::create_reply_like(&server, &reply.id.to_raw(), Some(10)).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 10);
});

test_with_server!(reply_likes, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let disc = DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc, None, None).await;
    let reply = create_fake_reply(server, &post.id).await;

    let response = post_helpers::create_reply_like(&server, &reply.id.to_raw(), None).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 1);

    let (server, user1, _, _) = create_fake_login_test_user(&server).await;
    let response = post_helpers::create_reply_like(&server, &reply.id.to_raw(), None).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;

    assert_eq!(likes_nr, 2);

    let replies = server
        .get(format!("/api/posts/{}/replies", post.id).as_str())
        .await
        .json::<Vec<ReplyView>>();

    assert_eq!(replies.len(), 1);
    let reply = replies.first().unwrap();
    let liked_by = reply.liked_by.clone().unwrap_or_default();
    assert!(liked_by.contains(user1.id.as_ref().unwrap()));
    assert!(!liked_by.contains(user.id.as_ref().unwrap()));
});
