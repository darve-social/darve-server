mod helpers;

use crate::helpers::create_fake_login_test_user;
use crate::helpers::post_helpers::create_fake_post;
use darve_server::entities::community::discussion_entity::DiscussionDbService;
use darve_server::middleware::utils::string_utils::get_string_thing;
use serde_json::json;

test_with_server!(get_tags_empty, |server, ctx_state, config| {
    let (server, _, _, _) = create_fake_login_test_user(&server).await;

    let response = server.get("/api/tags").await;

    response.assert_status_success();
    let tags = response.json::<Vec<String>>();
    assert_eq!(tags.len(), 0);
});

test_with_server!(
    get_tags_after_creating_posts_with_tags,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;

        let result = DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

        let tags1 = vec!["rust".to_string(), "programming".to_string()];
        let _ = create_fake_post(server, &result, None, Some(tags1)).await;

        let tags2 = vec!["javascript".to_string(), "web".to_string()];
        let _ = create_fake_post(server, &result, None, Some(tags2)).await;

        let tags3 = vec!["rust".to_string(), "backend".to_string()];
        let _ = create_fake_post(server, &result, None, Some(tags3)).await;

        let response = server.get("/api/tags").await;
        response.assert_status_success();
        let tags = response.json::<Vec<String>>();

        // We should have 4 unique tags: rust, programming, javascript, web, backend
        assert_eq!(tags.len(), 5);
        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"programming".to_string()));
        assert!(tags.contains(&"javascript".to_string()));
        assert!(tags.contains(&"web".to_string()));
        assert!(tags.contains(&"backend".to_string()));
    }
);

test_with_server!(get_tags_with_profile_posts, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();
    let user_thing_id = get_string_thing(user_ident).unwrap();

    let default_discussion = DiscussionDbService::get_profile_discussion_id(&user_thing_id);

    // Create post on profile with tags
    let tags = vec![
        "profile".to_string(),
        "personal".to_string(),
        "test".to_string(),
    ];
    let _ = create_fake_post(server, &default_discussion, None, Some(tags)).await;

    let response = server.get("/api/tags").await;

    response.assert_status_success();
    let tags = response.json::<Vec<String>>();

    assert_eq!(tags.len(), 3);
    assert!(tags.contains(&"profile".to_string()));
    assert!(tags.contains(&"personal".to_string()));
    assert!(tags.contains(&"test".to_string()));
});

test_with_server!(
    get_tags_with_special_characters,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;

        let result = DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

        // Create post with tags containing special characters
        let tags = vec![
            "tag-with-dash".to_string(),
            "tag_with_underscore".to_string(),
            "tag123".to_string(),
            "UPPERCASE".to_string(),
        ];
        let _ = create_fake_post(server, &result, None, Some(tags)).await;

        let response = server.get("/api/tags").await;

        response.assert_status_success();
        let tags = response.json::<Vec<String>>();

        assert_eq!(tags.len(), 4);
        assert!(tags.contains(&"tag-with-dash".to_string()));
        assert!(tags.contains(&"tag_with_underscore".to_string()));
        assert!(tags.contains(&"tag123".to_string()));
        assert!(tags.contains(&"UPPERCASE".to_string()));
    }
);

test_with_server!(
    get_tags_after_multiple_users,
    |server, ctx_state, config| {
        let (server, user1, _, _) = create_fake_login_test_user(&server).await;
        let result1 = DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());
        let tags1 = vec!["user1tag".to_string(), "shared".to_string()];
        let _ = create_fake_post(server, &result1, None, Some(tags1)).await;

        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let result2 = DiscussionDbService::get_profile_discussion_id(&user2.id.as_ref().unwrap());
        let tags2 = vec!["user2tag".to_string(), "shared".to_string()];
        let _ = create_fake_post(server, &result2, None, Some(tags2)).await;

        let response = server.get("/api/tags").await;

        response.assert_status_success();
        let tags = response.json::<Vec<String>>();

        // Should have 3 unique tags: user1tag, user2tag, shared
        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&"user1tag".to_string()));
        assert!(tags.contains(&"user2tag".to_string()));
        assert!(tags.contains(&"shared".to_string()));
    }
);

test_with_server!(get_sorted_by_most_likes, |server, state, config| {
    let (server, user, _, __) = create_fake_login_test_user(&server).await;
    let discussion_id = DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());
    let post_0 = create_fake_post(
        &server,
        &discussion_id,
        None,
        Some(vec![
            "js".to_string(),
            "rust".to_string(),
            "c++".to_string(),
        ]),
    )
    .await;

    let post_1 =
        create_fake_post(&server, &discussion_id, None, Some(vec!["js".to_string()])).await;
    let post_2 =
        create_fake_post(&server, &discussion_id, None, Some(vec!["js".to_string()])).await;

    let (server, _user1, _, __) = create_fake_login_test_user(&server).await;

    let like_response = server
        .post(&format!("/api/posts/{}/like", post_0.id))
        .json(&json!({"count": 10}))
        .await;
    like_response.assert_status_success();
    let like_response = server
        .post(&format!("/api/posts/{}/like", post_1.id))
        .json(&json!({"count": 1}))
        .await;
    like_response.assert_status_success();
    let like_response = server
        .post(&format!("/api/posts/{}/like", post_2.id))
        .json(&json!({"count": 1}))
        .await;
    like_response.assert_status_success();

    let follow_response = server
        .post(&format!(
            "/api/followers/{}",
            user.id.as_ref().unwrap().to_raw()
        ))
        .await;
    follow_response.assert_status_success();

    let response = server.get("/api/tags").await;
    response.assert_status_success();
    let data = response.json::<Vec<String>>();
    assert_eq!(data[0], "js");
    assert_eq!(data[1], "c++");
    assert_eq!(data[2], "rust");
});
