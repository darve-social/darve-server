mod helpers;

use crate::helpers::create_fake_login_test_user;
use darve_server::routes::posts::PostLikeResponse;
use helpers::community_helpers::create_fake_community;
use helpers::post_helpers::{self, create_fake_post};

test_with_server!(create_post_like, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();
    let result = create_fake_community(server, &ctx_state, user_ident.clone()).await;

    let result = create_fake_post(server, &result.default_discussion, None, None).await;

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
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();
    let result = create_fake_community(server, &ctx_state, user_ident.clone()).await;

    let result = create_fake_post(server, &result.default_discussion, None, None).await;

    // check like and number
    let response = post_helpers::create_post_like(&server, &result.id, Some(4)).await;
    response.assert_status_ok();
    let likes_nr = response.json::<PostLikeResponse>().likes_count;
    assert_eq!(likes_nr, 4);
});

test_with_server!(udpate_likes, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();
    let result = create_fake_community(server, &ctx_state, user_ident.clone()).await;
    let result = create_fake_post(server, &result.default_discussion, None, None).await;

    let response = post_helpers::create_post_like(&server, &result.id, None).await;
    response.assert_status_ok();
    let likes_nr = response.json::<PostLikeResponse>().likes_count;
    assert_eq!(likes_nr, 1);

    let response = post_helpers::create_post_like(&server, &result.id, Some(10)).await;
    response.assert_status_ok();
    let likes_nr = response.json::<PostLikeResponse>().likes_count;
    assert_eq!(likes_nr, 10);
});
