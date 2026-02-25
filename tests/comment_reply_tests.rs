mod helpers;
use darve_server::{
    entities::community::{
        community_entity::CommunityDbService,
        discussion_entity::{Discussion, DiscussionDbService},
    },
    models::view::reply::ReplyView,
    services::discussion_service::CreateDiscussion,
};
use serde_json::json;

use crate::helpers::{
    create_fake_login_test_user,
    post_helpers::{create_fake_post, create_reply_like, delete_reply_like},
};

test_with_server!(create_reply_for_comment, |server, ctx_state, config| {
    let (server, user_ident, _, token) = create_fake_login_test_user(&server).await;
    let disc_id = DiscussionDbService::get_profile_discussion_id(&user_ident.id.as_ref().unwrap());
    let created_post = create_fake_post(&server, &disc_id, None, None, &token).await;

    let create_comment_response = server
        .post(format!("/api/posts/{}/replies", created_post.uri).as_str())
        .json(&json!({
            "content": "This is a comment.",
        }))
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .await;
    create_comment_response.assert_status_success();
    let comment = create_comment_response.json::<ReplyView>();

    let create_reply_response = server
        .post(format!("/api/comments/{}/replies", comment.id.to_raw()).as_str())
        .json(&json!({
            "content": "This is a reply to the comment.",
        }))
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .await;
    create_reply_response.assert_status_success();
});

test_with_server!(get_replies_for_comment, |server, ctx_state, config| {
    let (server, user_ident, _, token) = create_fake_login_test_user(&server).await;
    let disc_id = DiscussionDbService::get_profile_discussion_id(&user_ident.id.as_ref().unwrap());
    let created_post = create_fake_post(&server, &disc_id, None, None, &token).await;

    let create_comment_response = server
        .post(format!("/api/posts/{}/replies", created_post.id).as_str())
        .json(&json!({
            "content": "This is a comment.",
        }))
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .await;
    create_comment_response.assert_status_success();
    let comment = create_comment_response.json::<ReplyView>();

    for i in 0..3 {
        let create_reply_response = server
            .post(format!("/api/comments/{}/replies", comment.id.to_raw()).as_str())
            .json(&json!({
                "content": format!("This is reply {}.", i),
            }))
            .add_header("Authorization", format!("Bearer {}", token))
            .add_header("Accept", "application/json")
            .await;
        create_reply_response.assert_status_success();
    }

    let get_replies_response = server
        .get(format!("/api/comments/{}/replies", comment.id.to_raw()).as_str())
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token))
        .await;
    get_replies_response.assert_status_success();

    let replies = get_replies_response.json::<Vec<ReplyView>>();
    assert_eq!(replies.len(), 3);

    let get_replies_response = server
        .get(format!("/api/posts/{}/replies", created_post.id).as_str())
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token))
        .await;
    get_replies_response.assert_status_success();

    let replies = get_replies_response.json::<Vec<ReplyView>>();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].replies_nr, 3);
});

test_with_server!(
    forbidden_create_reply_for_comment_in_priavet_disc,
    |server, ctx_state, config| {
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let (server, user, _, token) = create_fake_login_test_user(&server).await;
        let comm_id = CommunityDbService::get_profile_community_id(&user.id.as_ref().unwrap());

        let create_response = server
            .post("/api/discussions")
            .json(&CreateDiscussion {
                community_id: comm_id.to_raw(),
                title: "The Discussion".to_string(),
                image_uri: None,
                chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
                private_discussion_users_final: false,
            })
            .add_header("Authorization", format!("Bearer {}", token))
            .add_header("Accept", "application/json")
            .await;

        let disc_id = create_response.json::<Discussion>().id;
        let created_post = create_fake_post(&server, &disc_id, None, None, &token).await;

        let create_comment_response = server
            .post(format!("/api/posts/{}/replies", created_post.uri).as_str())
            .json(&json!({
                "content": "This is a comment.",
            }))
            .add_header("Authorization", format!("Bearer {}", token))
            .add_header("Accept", "application/json")
            .await;
        create_comment_response.assert_status_success();
        let comment = create_comment_response.json::<ReplyView>();

        let create_reply_response = server
            .post(format!("/api/comments/{}/replies", comment.id.to_raw()).as_str())
            .json(&json!({
                "content": "This is a reply to the comment.",
            }))
            .add_header("Authorization", format!("Bearer {}", token1))
            .add_header("Accept", "application/json")
            .await;
        create_reply_response.assert_status_forbidden();
    }
);

test_with_server!(like_reply_for_comment, |server, ctx_state, config| {
    let (server, user_ident, _, token) = create_fake_login_test_user(&server).await;
    let disc_id = DiscussionDbService::get_profile_discussion_id(&user_ident.id.as_ref().unwrap());
    let created_post = create_fake_post(&server, &disc_id, None, None, &token).await;

    let create_comment_response = server
        .post(format!("/api/posts/{}/replies", created_post.uri).as_str())
        .json(&json!({
            "content": "This is a comment.",
        }))
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .await;
    create_comment_response.assert_status_success();
    let comment = create_comment_response.json::<ReplyView>();

    let create_reply_response = server
        .post(format!("/api/comments/{}/replies", comment.id.to_raw()).as_str())
        .json(&json!({
            "content": "This is a reply to the comment.",
        }))
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .await;
    create_reply_response.assert_status_success();

    let reply = create_reply_response.json::<ReplyView>();
    let like_response = create_reply_like(server, &reply.id.to_raw(), None, &token).await;
    like_response.assert_status_success();
});

test_with_server!(unlike_reply_for_comment, |server, ctx_state, config| {
    let (server, user_ident, _, token) = create_fake_login_test_user(&server).await;
    let disc_id = DiscussionDbService::get_profile_discussion_id(&user_ident.id.as_ref().unwrap());
    let created_post = create_fake_post(&server, &disc_id, None, None, &token).await;

    let create_comment_response = server
        .post(format!("/api/posts/{}/replies", created_post.uri).as_str())
        .json(&json!({
            "content": "This is a comment.",
        }))
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .await;
    create_comment_response.assert_status_success();
    let comment = create_comment_response.json::<ReplyView>();

    let create_reply_response = server
        .post(format!("/api/comments/{}/replies", comment.id.to_raw()).as_str())
        .json(&json!({
            "content": "This is a reply to the comment.",
        }))
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .await;
    create_reply_response.assert_status_success();

    let reply = create_reply_response.json::<ReplyView>();

    let like_response = delete_reply_like(server, &reply.id.to_raw(), &token).await;
    like_response.assert_status_success();
});
