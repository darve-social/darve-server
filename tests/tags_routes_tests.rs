mod helpers;

use crate::helpers::community_helpers::create_fake_community;
use crate::helpers::create_fake_login_test_user;
use crate::helpers::post_helpers::create_fake_post;
use darve_server::entities::community::discussion_entity::DiscussionDbService;
use darve_server::middleware::utils::string_utils::get_string_thing;

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
        let user_ident = user.id.as_ref().unwrap().to_raw();

        let result = create_fake_community(server, &ctx_state, user_ident.clone()).await;

        // Create multiple posts with different tags
        let tags1 = vec!["rust".to_string(), "programming".to_string()];
        let _ = create_fake_post(server, &result.default_discussion, None, Some(tags1)).await;

        let tags2 = vec!["javascript".to_string(), "web".to_string()];
        let _ = create_fake_post(server, &result.default_discussion, None, Some(tags2)).await;

        let tags3 = vec!["rust".to_string(), "backend".to_string()];
        let _ = create_fake_post(server, &result.default_discussion, None, Some(tags3)).await;

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
        let user_ident = user.id.as_ref().unwrap().to_raw();

        let result = create_fake_community(server, &ctx_state, user_ident.clone()).await;

        // Create post with tags containing special characters
        let tags = vec![
            "tag-with-dash".to_string(),
            "tag_with_underscore".to_string(),
            "tag123".to_string(),
            "UPPERCASE".to_string(),
        ];
        let _ = create_fake_post(server, &result.default_discussion, None, Some(tags)).await;

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
        let user1_ident = user1.id.as_ref().unwrap().to_raw();

        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let user2_ident = user2.id.as_ref().unwrap().to_raw();

        // Create communities for both users
        let result1 = create_fake_community(server, &ctx_state, user1_ident.clone()).await;
        let result2 = create_fake_community(server, &ctx_state, user2_ident.clone()).await;

        // User 1 creates posts with tags
        let tags1 = vec!["user1tag".to_string(), "shared".to_string()];
        let _ = create_fake_post(server, &result1.default_discussion, None, Some(tags1)).await;

        // User 2 creates posts with tags
        let tags2 = vec!["user2tag".to_string(), "shared".to_string()];
        let _ = create_fake_post(server, &result2.default_discussion, None, Some(tags2)).await;

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
