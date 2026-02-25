mod helpers;
use crate::helpers::create_fake_login_test_user;
use crate::helpers::post_helpers::create_fake_post;
use community_entity::CommunityDbService;
use darve_server::entities::community::community_entity;
use darve_server::entities::community::discussion_entity::Discussion;
use darve_server::models::view::reply::ReplyView;
use darve_server::services::discussion_service::CreateDiscussion;
use serde_json::json;

test_with_server!(create_reply, |server, ctx_state, config| {
    let (server, user_ident, _, token) = create_fake_login_test_user(&server).await;
    let comm_id = CommunityDbService::get_profile_community_id(&user_ident.id.as_ref().unwrap());
    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            title: "The Community Test".to_string(),
            community_id: comm_id.to_raw(),
            image_uri: None,
            chat_user_ids: None,
            private_discussion_users_final: false,
        })
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_success();
    let created = &create_response.json::<Discussion>();

    let comm_disc_thing = created.id.clone();
    let comm_discussion_id = comm_disc_thing.to_raw();
    assert_eq!(comm_discussion_id.len() > 0, true);

    let created_post = create_fake_post(server, &comm_disc_thing, None, None, &token).await;

    let post_uri = &created_post.uri.clone();

    let create_response = server
        .post(format!("/api/posts/{post_uri}/replies").as_str())
        .json(&json!({
            "content": "contentttt222".to_string(),
        }))
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .await;
    dbg!(&create_response);

    let create_response2 = server
        .post(format!("/api/posts/{post_uri}/replies").as_str())
        .json(&json!({
            "content": "contentttt222".to_string(),
        }))
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .await;

    let create_response3 = server
        .post(format!("/api/posts/{post_uri}/replies").as_str())
        .json(&json!({
            "content": "contentttt222".to_string(),
        }))
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .await;

    let create_response4 = server
        .post(format!("/api/posts/{post_uri}/replies").as_str())
        .json(&json!({
            "content": "contentttt222".to_string(),
        }))
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_success();
    create_response2.assert_status_success();
    create_response3.assert_status_success();
    create_response4.assert_status_success();

    let _ = create_response.json::<ReplyView>();
    let _ = create_response3.json::<ReplyView>();
});
