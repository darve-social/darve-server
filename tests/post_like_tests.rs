mod helpers;

use crate::helpers::create_fake_login_test_user;
use darve_server::{
    entities::community::discussion_entity::DiscussionDbService,
    middleware::utils::string_utils::get_string_thing, models::view::post::PostView,
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
    let (server, user1, _, _) = create_fake_login_test_user(&server).await;
    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();
    let result =
        DiscussionDbService::get_profile_discussion_id(&get_string_thing(user_ident).unwrap());

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
    let result = create_fake_post(server, &result, None, None).await;

    server
        .post(format!("/api/posts/{}/tasks", result.id).as_str())
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
    // check like and number
    let response = post_helpers::create_post_like(&server, &result.id, Some(4)).await;
    response.assert_status_ok();
    let likes_nr = response.json::<PostLikeResponse>().likes_count;
    assert_eq!(likes_nr, 4);
});

test_with_server!(update_likes, |server, ctx_state, config| {
    let (server, user1, _, _) = create_fake_login_test_user(&server).await;
    let (server, user, _, token) = create_fake_login_test_user(&server).await;
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

    let user_ident = user.id.as_ref().unwrap().to_raw();
    let result =
        DiscussionDbService::get_profile_discussion_id(&get_string_thing(user_ident).unwrap());
    let result = create_fake_post(server, &result, None, None).await;

    let response = post_helpers::create_post_like(&server, &result.id, None).await;
    response.assert_status_ok();
    let likes_nr = response.json::<PostLikeResponse>().likes_count;
    assert_eq!(likes_nr, 1);

    let response = post_helpers::create_post_like(&server, &result.id, Some(10)).await;
    response.assert_status_forbidden();

    server
        .post(format!("/api/posts/{}/tasks", result.id).as_str())
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

    let response = post_helpers::create_post_like(&server, &result.id, Some(10)).await;
    response.assert_status_ok();
    let likes_nr = response.json::<PostLikeResponse>().likes_count;
    assert_eq!(likes_nr, 10);
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
