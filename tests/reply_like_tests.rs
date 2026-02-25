mod helpers;

use crate::helpers::create_fake_login_test_user;
use crate::helpers::post_helpers::create_fake_reply;
use darve_server::entities::user_auth::local_user_entity::LocalUserDbService;
use darve_server::middleware::ctx::Ctx;
use darve_server::routes::reply::LikeResponse;
use darve_server::{
    entities::community::discussion_entity::DiscussionDbService, models::view::reply::ReplyView,
};
use helpers::post_helpers::{self, create_fake_post};

test_with_server!(create_reply_like, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let discussion = DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap());
    let post = create_fake_post(server, &discussion, None, None, &token).await;
    let reply = create_fake_reply(server, &post.id, &token).await;
    // check like and number
    let response = post_helpers::create_reply_like(&server, &reply.id.to_raw(), None, &token).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 1);

    // check delete and count
    let response = post_helpers::delete_reply_like(&server, &reply.id.to_raw(), &token).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 0);

    // throw if like post that does not exist
    let response =
        post_helpers::create_reply_like(&server, "post:that_does_not_exist", None, &token).await;
    response.assert_status_failure();
});

test_with_server!(create_reply_like_with_count, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let discussion = DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap());
    let post = create_fake_post(server, &discussion, None, None, &token).await;
    let reply = create_fake_reply(server, &post.id, &token).await;
    let count = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok(user.id.as_ref().unwrap().to_raw()), false),
    }
    .add_credits(user.id.as_ref().unwrap().clone(), 100)
    .await
    .unwrap();

    assert_eq!(count, 100);
    let response =
        post_helpers::create_reply_like(&server, &reply.id.to_raw(), Some(4), &token).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 4);
});

test_with_server!(update_likes, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let discussion = DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap());
    let post = create_fake_post(server, &discussion, None, None, &token).await;
    let reply = create_fake_reply(server, &post.id, &token).await;

    let response = post_helpers::create_reply_like(&server, &reply.id.to_raw(), None, &token).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 1);

    let response =
        post_helpers::create_reply_like(&server, &reply.id.to_raw(), Some(10), &token).await;
    response.assert_status_failure();

    let count = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok(user.id.as_ref().unwrap().to_raw()), false),
    }
    .add_credits(user.id.as_ref().unwrap().clone(), 100)
    .await
    .unwrap();

    assert_eq!(count, 100);

    let response =
        post_helpers::create_reply_like(&server, &reply.id.to_raw(), Some(10), &token).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 10);
});

test_with_server!(reply_likes, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let disc = DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc, None, None, &token).await;
    let reply = create_fake_reply(server, &post.id, &token).await;

    let response = post_helpers::create_reply_like(&server, &reply.id.to_raw(), None, &token).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 1);

    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let response =
        post_helpers::create_reply_like(&server, &reply.id.to_raw(), None, &token1).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;

    assert_eq!(likes_nr, 2);

    let replies = server
        .get(format!("/api/posts/{}/replies", post.id).as_str())
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await
        .json::<Vec<ReplyView>>();

    assert_eq!(replies.len(), 1);
    let reply = replies.first().unwrap();
    let liked_by = reply.liked_by.clone().unwrap_or_default();
    assert!(liked_by.contains(user1.id.as_ref().unwrap()));
    assert!(!liked_by.contains(user.id.as_ref().unwrap()));
});

test_with_server!(try_gives_100_likes_to_reply, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let result = DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());
    let post = create_fake_post(server, &result, None, None, &token).await;
    let reply = create_fake_reply(server, &post.id, &token).await;
    let response =
        post_helpers::create_reply_like(&server, &reply.id.to_raw(), Some(100), &token).await;
    response.assert_status_failure();
});

test_with_server!(try_gives_1_likes_to_reply, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let result = DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());
    let post = create_fake_post(server, &result, None, None, &token).await;
    let reply = create_fake_reply(server, &post.id, &token).await;
    let response =
        post_helpers::create_reply_like(&server, &reply.id.to_raw(), Some(1), &token).await;
    response.assert_status_failure();
});
