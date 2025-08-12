mod helpers;

use crate::helpers::create_fake_login_test_user;
use crate::helpers::post_helpers::create_fake_reply;
use darve_server::entities::community::discussion_entity::DiscussionDbService;
use darve_server::routes::reply::LikeResponse;
use helpers::post_helpers::{self, create_fake_post};

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
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let discussion = DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap());
    let post = create_fake_post(server, &discussion, None, None).await;
    let reply = create_fake_reply(server, &post.id).await;
    // check like and number
    let response = post_helpers::create_reply_like(&server, &reply.id.to_raw(), Some(4)).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 4);
});

test_with_server!(udpate_likes, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let discussion = DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap());
    let post = create_fake_post(server, &discussion, None, None).await;
    let reply = create_fake_reply(server, &post.id).await;

    let response = post_helpers::create_reply_like(&server, &reply.id.to_raw(), None).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 1);

    let response = post_helpers::create_reply_like(&server, &reply.id.to_raw(), Some(10)).await;
    response.assert_status_ok();
    let likes_nr = response.json::<LikeResponse>().likes_count;
    assert_eq!(likes_nr, 10);
});
