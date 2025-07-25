mod helpers;

use crate::helpers::post_helpers::create_post;
use crate::helpers::{create_fake_login_test_user, post_helpers};
use axum::extract::Query;
use axum::extract::{Path, State};
use axum_test::multipart::MultipartForm;
use community_entity::CommunityDbService;
use community_routes::get_community;
use darve_server::entities::community::community_entity;
use darve_server::entities::community::discussion_entity::DiscussionDbService;
use darve_server::entities::community::post_entity::Post;
use darve_server::entities::community::post_entity::PostDbService;
use darve_server::middleware::utils::db_utils::RecordWithId;
use darve_server::middleware::utils::string_utils::get_string_thing;
use darve_server::middleware::{self};
use darve_server::routes::community::community_routes;
use darve_server::routes::community::profile_routes::get_profile_community;
use darve_server::routes::posts::GetPostsQuery;
use helpers::community_helpers::create_fake_community;
use helpers::community_helpers::get_profile_discussion_id;
use helpers::community_helpers::{self, create_private_discussion};
use helpers::post_helpers::get_posts;
use helpers::post_helpers::{
    create_fake_post, create_fake_post_with_file, create_fake_post_with_large_file, hide_post,
    show_post,
};
use middleware::ctx::Ctx;
use middleware::utils::extractor_utils::DiscussionParams;

test_with_server!(create_post_test, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();

    let result = create_fake_community(server, &ctx_state, user_ident.clone()).await;

    let ctx = Ctx::new(Ok(user_ident), false);

    let _ = create_fake_post(server, &result.default_discussion, None, None).await;
    let _ = create_fake_post(server, &result.default_discussion, None, None).await;
    let _ = create_fake_post(server, &result.default_discussion, None, None).await;
    let _ = create_fake_post(server, &result.default_discussion, None, None).await;

    let comm_view = get_community(
        State(ctx_state.clone()),
        ctx,
        Path(result.name),
        Query(DiscussionParams {
            topic_id: None,
            start: None,
            count: None,
        }),
    )
    .await
    .expect("community page");
    let posts = comm_view
        .community_view
        .unwrap()
        .discussion_view
        .unwrap()
        .posts;
    assert_eq!(posts.len(), 4);
});

test_with_server!(
    create_post_with_the_same_name,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let user_ident = user.id.as_ref().unwrap().to_raw();

        let result = create_fake_community(server, &ctx_state, user_ident.clone()).await;

        let title = "TEST_TEST";
        let data = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content");

        let response =
            helpers::post_helpers::create_post(server, &result.default_discussion, data).await;

        response.assert_status_success();

        let data_1 = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content");
        let response_1 =
            helpers::post_helpers::create_post(server, &result.default_discussion, data_1).await;

        response_1.assert_status_bad_request();
    }
);

test_with_server!(create_post_with_file_test, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();

    let result = get_profile_discussion_id(server, user_ident.clone()).await;
    let _ = create_fake_post_with_large_file(server, &ctx_state, &result).await;
    let _ = create_fake_post_with_file(server, &ctx_state, &result).await;

    let posts_res = get_posts(&server, None).await;
    posts_res.assert_status_success();
    let posts = posts_res.json::<Vec<Post>>();
    let post = posts.last().unwrap();
    assert_eq!(post.media_links.as_ref().unwrap().len(), 1);
    assert!(post.media_links.as_ref().unwrap()[0].contains("test_image_2mb.jpg"));
});

test_with_server!(get_latest, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();
    let ctx = Ctx::new(Ok(user_ident.clone()), false);
    let user_thing_id = get_string_thing(user_ident).unwrap();

    let default_discussion =
        get_profile_community(&ctx_state.db.client, &ctx, user_thing_id.clone())
            .await
            .unwrap()
            .default_discussion
            .unwrap();
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;

    let profile_comm = CommunityDbService {
        ctx: &ctx,
        db: &ctx_state.db.client,
    }
    .get_profile_community(user_thing_id)
    .await;
    let discussion_id = profile_comm.unwrap().default_discussion.unwrap();
    let result = PostDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .get_by_discussion_desc_view::<RecordWithId>(
        discussion_id.clone(),
        DiscussionParams {
            topic_id: None,
            start: Some(0),
            count: Some(2),
        },
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 2);

    let result = PostDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .get_by_discussion_desc_view::<RecordWithId>(
        discussion_id.clone(),
        DiscussionParams {
            topic_id: None,
            start: Some(0),
            count: Some(3),
        },
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 3);
    let result = PostDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .get_by_discussion_desc_view::<RecordWithId>(
        discussion_id.clone(),
        DiscussionParams {
            topic_id: None,
            start: Some(0),
            count: Some(1),
        },
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1)
});

test_with_server!(create_post_with_tags, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();

    let default_discussion =
        community_helpers::get_profile_discussion_id(server, user_ident.clone()).await;

    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let tags = vec![
        "tag".to_string(),
        "tag1".to_string(),
        "tag2".to_string(),
        "tag3".to_string(),
        "tag4".to_string(),
        "tag5".to_string(),
    ];
    let _ = create_fake_post(
        server,
        &default_discussion,
        None,
        Some(Vec::from(&tags[0..5])),
    )
    .await;
    let posts_res = get_posts(&server, None).await;
    posts_res.assert_status_success();
    let posts = posts_res.json::<Vec<Post>>();
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0].tags.as_ref().unwrap()[0], tags[0]);
    assert_eq!(posts[0].tags.as_ref().unwrap()[1], tags[1]);
    assert_eq!(posts[1].tags, None);
    let data = post_helpers::build_fake_post(None, Some(tags.clone()));
    let response = post_helpers::create_post(server, &default_discussion, data).await;
    response.assert_status_unprocessable_entity();
});

test_with_server!(filter_posts_by_tag, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();
    let default_discussion =
        community_helpers::get_profile_discussion_id(server, user_ident.clone()).await;
    let tags = vec!["tag".to_string(), "tag1".to_string()];

    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(
        server,
        &default_discussion,
        None,
        Some(vec![tags[0].clone()]),
    )
    .await;
    let _ = create_fake_post(server, &default_discussion, None, Some(tags.clone())).await;
    let _ = create_fake_post(
        server,
        &default_discussion,
        None,
        Some(vec!["non_of_them".to_string()]),
    )
    .await;

    let posts_res = get_posts(&server, None).await;
    posts_res.assert_status_success();
    let posts = posts_res.json::<Vec<Post>>();
    assert_eq!(posts.len(), 4);

    let posts_res = get_posts(
        &server,
        Some(GetPostsQuery {
            tag: Some(tags[0].clone()),
            start: None,
            count: None,
            order_dir: None,
        }),
    )
    .await;

    posts_res.assert_status_success();
    let posts = posts_res.json::<Vec<Post>>();
    assert_eq!(posts.len(), 2);

    let posts_res = get_posts(
        &server,
        Some(GetPostsQuery {
            tag: Some(tags[1].clone()),
            start: None,
            count: None,
            order_dir: None,
        }),
    )
    .await;

    posts_res.assert_status_success();
    let posts = posts_res.json::<Vec<Post>>();
    assert_eq!(posts.len(), 1);

    let posts_res = get_posts(
        &server,
        Some(GetPostsQuery {
            tag: Some("rust".to_string()),
            start: None,
            count: None,
            order_dir: None,
        }),
    )
    .await;

    posts_res.assert_status_success();
    let posts = posts_res.json::<Vec<Post>>();
    assert_eq!(posts.len(), 0);

    let posts_res = get_posts(
        &server,
        Some(GetPostsQuery {
            tag: None,
            start: None,
            count: Some(1),
            order_dir: None,
        }),
    )
    .await;

    posts_res.assert_status_success();
    let posts = posts_res.json::<Vec<Post>>();
    assert_eq!(posts.len(), 1);
});

test_with_server!(
    try_to_create_without_content_and_file,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let user_ident = user.id.as_ref().unwrap().to_raw();
        let default_discussion =
            community_helpers::get_profile_discussion_id(server, user_ident.clone()).await;

        let data = MultipartForm::new().add_text("title", "Hello");
        let response = create_post(server, &default_discussion, data).await;

        response.assert_status_failure();
        assert!(response.text().contains("Empty content and missing file"))
    }
);

test_with_server!(
    hide_post_from_self_should_fail,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let user_ident = user.id.as_ref().unwrap().to_raw();

        let default_discussion =
            community_helpers::get_profile_discussion_id(server, user_ident.clone()).await;

        let post_response = create_fake_post(server, &default_discussion, None, None).await;
        let post_id = post_response.id;

        // Test hiding the post from self - should fail with forbidden
        let hide_response = hide_post(server, &post_id, vec![user_ident.clone()]).await;
        hide_response.assert_status_forbidden();
    }
);

test_with_server!(
    show_post_from_self_should_fail,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let user_ident = user.id.as_ref().unwrap().to_raw();

        let default_discussion =
            community_helpers::get_profile_discussion_id(server, user_ident.clone()).await;

        let post_response = create_fake_post(server, &default_discussion, None, None).await;
        let post_id = post_response.id;

        // Test showing the post from self - should fail with forbidden
        let show_response = show_post(server, &post_id, vec![user_ident.clone()]).await;
        show_response.assert_status_forbidden();
    }
);

test_with_server!(
    hide_post_on_public_discussion_should_fail,
    |server, ctx_state, config| {
        let (server, user1, _, _) = create_fake_login_test_user(&server).await;
        let user1_ident = user1.id.as_ref().unwrap().to_raw();

        // Create another user
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let user2_ident = user2.id.as_ref().unwrap().to_raw();

        // Create a community (public discussion)
        let result = create_fake_community(server, &ctx_state, user1_ident.clone()).await;

        let post_response = create_fake_post(server, &result.default_discussion, None, None).await;
        let post_id = post_response.id;

        // Test hiding the post in public discussion - should fail
        let hide_response = hide_post(server, &post_id, vec![user2_ident.clone()]).await;
        hide_response.assert_status_forbidden();
    }
);

test_with_server!(
    try_to_hide_post_with_empty_user_ids_in_default_disc,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;

        let default_discussion =
            DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

        let post_response = create_fake_post(server, &default_discussion, None, None).await;
        let post_id = post_response.id;

        // Test hiding the post with empty user_ids list - should succeed (no-op)
        let hide_response = hide_post(server, &post_id, vec![]).await;
        hide_response.assert_status_forbidden();
    }
);
test_with_server!(
    hide_post_with_empty_user_ids,
    |server, ctx_state, config| {
        let (server, user, _, token1) = create_fake_login_test_user(&server).await;

        let discussion =
            create_private_discussion(&server, user.id.as_ref().unwrap().clone(), vec![], token1)
                .await;

        let post_response = create_fake_post(server, &discussion.id, None, None).await;
        let post_id = post_response.id;

        // Test hiding the post with empty user_ids list - should succeed (no-op)
        let hide_response = hide_post(server, &post_id, vec![]).await;
        hide_response.assert_status_ok();
    }
);

test_with_server!(hide_show_nonexistent_post, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();

    // Try to hide a non-existent post
    let fake_post_id = "post:nonexistent";
    let hide_response = hide_post(server, fake_post_id, vec![user_ident.clone()]).await;
    hide_response.assert_status_failure();

    // Try to show a non-existent post
    let show_response = show_post(server, fake_post_id, vec![user_ident.clone()]).await;
    show_response.assert_status_failure();
});

test_with_server!(
    non_owner_cannot_hide_show_post,
    |server, ctx_state, config| {
        let (server, user1, _, _) = create_fake_login_test_user(&server).await;
        let user1_ident = user1.id.as_ref().unwrap().to_raw();

        let default_discussion =
            community_helpers::get_profile_discussion_id(server, user1_ident.clone()).await;

        let post_response = create_fake_post(server, &default_discussion, None, None).await;
        let post_id = post_response.id;
        // Create another user
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let user2_ident = user2.id.as_ref().unwrap().to_raw();

        // User2 tries to hide user1's post - should fail with forbidden
        let hide_response = hide_post(server, &post_id, vec![user2_ident.clone()]).await;
        hide_response.assert_status_forbidden();

        // User2 tries to show user1's post - should fail with forbidden
        let show_response = show_post(server, &post_id, vec![user2_ident.clone()]).await;
        show_response.assert_status_forbidden();
    }
);

test_with_server!(
    hide_post_in_private_discussion_success,
    |server, ctx_state, config| {
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let user2_ident = user2.id.as_ref().unwrap().to_raw();

        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let user1_ident = user1.id.as_ref().unwrap();

        // Create a private discussion between user1 and user2
        let private_discussion = community_helpers::create_private_discussion(
            server,
            user1_ident.clone(),
            vec![user2_ident.clone()],
            token1,
        )
        .await;

        // User1 creates a post in the private discussion
        let post_response = create_fake_post(server, &private_discussion.id, None, None).await;
        let post_id = post_response.id;

        // User1 hides the post from user2 - should succeed
        let hide_response = hide_post(server, &post_id, vec![user2_ident.clone()]).await;
        hide_response.assert_status_success();
    }
);

test_with_server!(
    show_post_in_private_discussion_success,
    |server, ctx_state, config| {
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let user2_ident = user2.id.as_ref().unwrap().to_raw();

        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let user1_ident = user1.id.as_ref().unwrap();

        // Create a private discussion between user1 and user2
        let private_discussion = community_helpers::create_private_discussion(
            server,
            user1_ident.clone(),
            vec![user2_ident.clone()],
            token1,
        )
        .await;

        // User1 creates a post in the private discussion
        let post_response = create_fake_post(server, &private_discussion.id, None, None).await;
        let post_id = post_response.id;

        // First hide the post from user2
        let hide_response = hide_post(server, &post_id, vec![user2_ident.clone()]).await;
        hide_response.assert_status_success();

        // Then show the post to user2 again - should succeed
        let show_response = show_post(server, &post_id, vec![user2_ident.clone()]).await;
        show_response.assert_status_success();
    }
);

test_with_server!(
    hide_post_from_multiple_users_in_private_discussion,
    |server, ctx_state, config| {
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let user2_ident = user2.id.as_ref().unwrap().to_raw();

        let (server, user3, _, _) = create_fake_login_test_user(&server).await;
        let user3_ident = user3.id.as_ref().unwrap().to_raw();

        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let user1_ident = user1.id.as_ref().unwrap();

        // Create a private discussion with multiple participants
        let private_discussion = community_helpers::create_private_discussion(
            server,
            user1_ident.clone(),
            vec![user2_ident.clone(), user3_ident.clone()],
            token1,
        )
        .await;

        // User1 creates a post in the private discussion
        let post_response = create_fake_post(server, &private_discussion.id, None, None).await;
        let post_id = post_response.id;

        // User1 hides the post from both user2 and user3 - should succeed
        let hide_response = hide_post(
            server,
            &post_id,
            vec![user2_ident.clone(), user3_ident.clone()],
        )
        .await;
        hide_response.assert_status_success();
    }
);

test_with_server!(
    hide_post_from_non_participant_should_fail,
    |server, ctx_state, config| {
        let (server, user3, _, _) = create_fake_login_test_user(&server).await;
        let user3_ident = user3.id.as_ref().unwrap().to_raw();
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let user2_ident = user2.id.as_ref().unwrap().to_raw();
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let user1_ident = user1.id.as_ref().unwrap();

        // Create a private discussion between user1 and user2 only
        let private_discussion = community_helpers::create_private_discussion(
            server,
            user1_ident.clone(),
            vec![user2_ident.clone()],
            token1,
        )
        .await;

        // User1 creates a post in the private discussion
        let post_response = create_fake_post(server, &private_discussion.id, None, None).await;
        let post_id = post_response.id;

        // User1 tries to hide the post from user3 (who is not a participant) - should fail
        let hide_response = hide_post(server, &post_id, vec![user3_ident.clone()]).await;
        hide_response.assert_status_forbidden();
    }
);

test_with_server!(
    create_post_with_hidden_for_field_notifications_test,
    |server, ctx_state, config| {
        use darve_server::entities::user_notification::UserNotification;
        use darve_server::services::post_service::{PostInput, PostService};
        use middleware::ctx::Ctx;

        let (server, user2, _, token2) = create_fake_login_test_user(&server).await;
        let user2_ident = user2.id.as_ref().unwrap().to_raw();

        let (server, user3, _, token3) = create_fake_login_test_user(&server).await;
        let user3_ident = user3.id.as_ref().unwrap().to_raw();

        let (server, user4, _, token4) = create_fake_login_test_user(&server).await;
        let user4_ident = user4.id.as_ref().unwrap().to_raw();

        // Create 4 test users
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let user1_ident = user1.id.as_ref().unwrap().clone();

        // Create a private discussion with all 4 users
        let private_discussion = community_helpers::create_private_discussion(
            server,
            user1_ident.clone(),
            vec![
                user2_ident.clone(),
                user3_ident.clone(),
                user4_ident.clone(),
            ],
            token1.clone(),
        )
        .await;

        // User1 creates a post using PostService with hidden_for containing user2 and user3
        let ctx = Ctx::new(Ok(user1_ident.to_raw()), false);
        let post_service = PostService::new(
            &ctx_state.db.client,
            &ctx,
            &ctx_state.event_sender,
            &ctx_state.db.user_notifications,
            &ctx_state.file_storage,
        );

        let post_input = PostInput {
            title: "Test Post with Hidden Users".to_string(),
            content: Some("This post should be hidden from user2 and user3".to_string()),
            topic_id: None,
            tags: vec![],
            file_1: None,
            hidden_for: vec![user2_ident.clone(), user3_ident.clone()], // Hide from user2 and user3
        };

        let post = post_service
            .create(
                &user1_ident.id.to_raw(),
                &private_discussion.id.to_raw(),
                post_input,
            )
            .await
            .expect("Failed to create post");

        // Verify the post was created with correct hidden_for field
        assert!(post.hidden_for.is_some());
        let hidden_for = post.hidden_for.unwrap();
        assert_eq!(hidden_for.len(), 2);
        assert!(hidden_for.iter().any(|id| id.to_raw() == user2_ident));
        assert!(hidden_for.iter().any(|id| id.to_raw() == user3_ident));

        // Check notifications for user2 (should be hidden - no notifications)
        let user2_notifications_response = server
            .get("/api/notifications")
            .add_header("Cookie", format!("jwt={}", token2))
            .await;
        user2_notifications_response.assert_status_success();
        let user2_notifications = user2_notifications_response.json::<Vec<UserNotification>>();

        // User2 should not receive chat message notification since they are in hidden_for
        let chat_notifications: Vec<_> = user2_notifications
            .iter()
            .filter(|n| n.event.as_str() == "UserChatMessage")
            .collect();
        assert_eq!(
            chat_notifications.len(),
            0,
            "User2 should not receive notifications when hidden"
        );

        // Check notifications for user3 (should be hidden - no notifications)
        let user3_notifications_response = server
            .get("/api/notifications")
            .add_header("Cookie", format!("jwt={}", token3))
            .await;
        user3_notifications_response.assert_status_success();
        let user3_notifications = user3_notifications_response.json::<Vec<UserNotification>>();

        // User3 should not receive chat message notification since they are in hidden_for
        let chat_notifications: Vec<_> = user3_notifications
            .iter()
            .filter(|n| n.event.as_str() == "UserChatMessage")
            .collect();
        assert_eq!(
            chat_notifications.len(),
            0,
            "User3 should not receive notifications when hidden"
        );

        // Check notifications for user4 (should receive notifications)
        let user4_notifications_response = server
            .get("/api/notifications")
            .add_header("Cookie", format!("jwt={}", token4))
            .await;
        user4_notifications_response.assert_status_success();
        let user4_notifications = user4_notifications_response.json::<Vec<UserNotification>>();

        // User4 should receive chat message notification since they are NOT in hidden_for
        let chat_notifications: Vec<_> = user4_notifications
            .iter()
            .filter(|n| n.event.as_str() == "UserChatMessage")
            .collect();
        assert_eq!(
            chat_notifications.len(),
            1,
            "User4 should receive notifications when not hidden"
        );

        // Verify the notification content for user4
        let notification = chat_notifications.first().unwrap();
        assert_eq!(notification.created_by, user1_ident.id.to_raw());
        assert_eq!(notification.event.as_str(), "UserChatMessage");

        // Test creating another post without hidden_for - all users should get notifications
        let post_input_no_hidden = PostInput {
            title: "Test Post without Hidden Users".to_string(),
            content: Some("This post should be visible to all users".to_string()),
            topic_id: None,
            tags: vec![],
            file_1: None,
            hidden_for: vec![], // No hidden users
        };

        let _post2 = post_service
            .create(
                &user1_ident.id.to_raw(),
                &private_discussion.id.to_raw(),
                post_input_no_hidden,
            )
            .await
            .expect("Failed to create second post");

        // Wait for notifications to be processed
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Now all users should have received the second notification
        let user2_notifications_response2 = server
            .get("/api/notifications")
            .add_header("Cookie", format!("jwt={}", token2))
            .await;
        user2_notifications_response2.assert_status_success();
        let user2_notifications2 = user2_notifications_response2.json::<Vec<UserNotification>>();

        let chat_notifications2: Vec<_> = user2_notifications2
            .iter()
            .filter(|n| n.event.as_str() == "UserChatMessage")
            .collect();
        assert_eq!(
            chat_notifications2.len(),
            1,
            "User2 should receive notification for second post"
        );

        let user3_notifications_response2 = server
            .get("/api/notifications")
            .add_header("Cookie", format!("jwt={}", token3))
            .await;
        user3_notifications_response2.assert_status_success();
        let user3_notifications2 = user3_notifications_response2.json::<Vec<UserNotification>>();

        let chat_notifications3: Vec<_> = user3_notifications2
            .iter()
            .filter(|n| n.event.as_str() == "UserChatMessage")
            .collect();
        assert_eq!(
            chat_notifications3.len(),
            1,
            "User3 should receive notification for second post"
        );

        let user4_notifications_response2 = server
            .get("/api/notifications")
            .add_header("Cookie", format!("jwt={}", token4))
            .await;
        user4_notifications_response2.assert_status_success();
        let user4_notifications2 = user4_notifications_response2.json::<Vec<UserNotification>>();

        let chat_notifications4: Vec<_> = user4_notifications2
            .iter()
            .filter(|n| n.event.as_str() == "UserChatMessage")
            .collect();
        assert_eq!(
            chat_notifications4.len(),
            2,
            "User4 should receive notifications for both posts"
        );
    }
);
