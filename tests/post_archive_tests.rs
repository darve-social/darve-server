mod helpers;
use crate::helpers::create_fake_login_test_user;
use darve_server::entities::community::discussion_entity;
use darve_server::models::view::PostView;
use discussion_entity::DiscussionDbService;
use helpers::community_helpers;
use helpers::post_helpers::{archive_post, create_fake_post, unarchive_post};

test_with_server!(archive_post_success, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap();

    let default_discussion = DiscussionDbService::get_profile_discussion_id(&user_ident);

    // Create a post
    let post_response = create_fake_post(server, &default_discussion, None, None).await;

    // Archive the post
    let archive_response = archive_post(server, &post_response.id).await;

    archive_response.assert_status_success();

    let res = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();

    assert_eq!(res.len(), 0);
});

test_with_server!(unarchive_post_success, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap();

    let default_discussion = DiscussionDbService::get_profile_discussion_id(&user_ident);

    // Create a post
    let post_response = create_fake_post(server, &default_discussion, None, None).await;

    // Archive the post first
    let archive_response = archive_post(server, &post_response.id).await;
    archive_response.assert_status_success();

    // Unarchive the post
    let unarchive_response = unarchive_post(server, &post_response.id).await;

    unarchive_response.assert_status_success();

    let res = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();

    assert_eq!(res.len(), 1);
});

test_with_server!(
    archive_nonexistent_post_fails,
    |server, ctx_state, config| {
        let (server, _user, _, _) = create_fake_login_test_user(&server).await;

        // Try to archive a non-existent post
        let fake_post_id = "post:nonexistent";
        let archive_response = archive_post(server, fake_post_id).await;

        archive_response.assert_status_failure();
    }
);

test_with_server!(
    unarchive_nonexistent_post_fails,
    |server, ctx_state, config| {
        let (server, _user, _, _) = create_fake_login_test_user(&server).await;

        // Try to unarchive a non-existent post
        let fake_post_id = "post:nonexistent";
        let unarchive_response = unarchive_post(server, fake_post_id).await;

        unarchive_response.assert_status_failure();
    }
);

test_with_server!(
    archive_same_post_multiple_times,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let user_ident = user.id.as_ref().unwrap();

        let default_discussion = DiscussionDbService::get_profile_discussion_id(&user_ident);

        // Create a post
        let post_response = create_fake_post(server, &default_discussion, None, None).await;

        // Archive the post first time
        let archive_response1 = archive_post(server, &post_response.id).await;
        archive_response1.assert_status_success();

        // Archive the same post second time - should still succeed (idempotent)
        let archive_response2 = archive_post(server, &post_response.id).await;
        archive_response2.assert_status_failure();
    }
);

test_with_server!(
    unarchive_same_post_multiple_times,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let user_ident = user.id.as_ref().unwrap();

        let default_discussion = DiscussionDbService::get_profile_discussion_id(&user_ident);

        // Create a post
        let post_response = create_fake_post(server, &default_discussion, None, None).await;

        // Archive the post first
        let archive_response = archive_post(server, &post_response.id).await;
        archive_response.assert_status_success();

        // Unarchive the post first time
        let unarchive_response1 = unarchive_post(server, &post_response.id).await;
        unarchive_response1.assert_status_success();

        // Unarchive the same post second time - should still succeed (idempotent)
        let unarchive_response2 = unarchive_post(server, &post_response.id).await;
        unarchive_response2.assert_status_success();
    }
);

test_with_server!(archive_unarchive_workflow, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap();

    let default_discussion = DiscussionDbService::get_profile_discussion_id(&user_ident);

    // Create multiple posts
    let post1 = create_fake_post(server, &default_discussion, None, None).await;
    let post2 = create_fake_post(server, &default_discussion, None, None).await;
    let post3 = create_fake_post(server, &default_discussion, None, None).await;

    // Archive first two posts
    let archive1_response = archive_post(server, &post1.id).await;
    archive1_response.assert_status_success();

    let archive2_response = archive_post(server, &post2.id).await;
    archive2_response.assert_status_success();

    // Unarchive the first post
    let unarchive1_response = unarchive_post(server, &post1.id).await;
    unarchive1_response.assert_status_success();

    // Archive the third post
    let archive3_response = archive_post(server, &post3.id).await;
    archive3_response.assert_status_success();

    // Unarchive all remaining archived posts
    let unarchive2_response = unarchive_post(server, &post2.id).await;
    unarchive2_response.assert_status_success();

    let unarchive3_response = unarchive_post(server, &post3.id).await;
    unarchive3_response.assert_status_success();
});

test_with_server!(
    different_users_can_archive_same_post,
    |server, ctx_state, config| {
        // Create first user and post
        let (server, user1, _, _) = create_fake_login_test_user(&server).await;
        let user1_ident = user1.id.as_ref().unwrap().to_raw();

        let default_discussion =
            community_helpers::get_profile_discussion_id(server, user1_ident.clone()).await;

        let post_response = create_fake_post(server, &default_discussion, None, None).await;

        // First user archives the post
        let archive1_response = archive_post(server, &post_response.id).await;
        archive1_response.assert_status_success();

        let (server, _user2, _, _) = create_fake_login_test_user(&server).await;

        // Second user can also archive the same post
        let archive2_response = archive_post(server, &post_response.id).await;
        archive2_response.assert_status_success();

        // Second user can unarchive their own archive
        let unarchive2_response = unarchive_post(server, &post_response.id).await;
        unarchive2_response.assert_status_success();
    }
);

test_with_server!(
    archive_post_with_invalid_post_id_format,
    |server, ctx_state, config| {
        let (server, _user, _, _) = create_fake_login_test_user(&server).await;

        // Try to archive with malformed post ID
        let invalid_post_id = "invalid-post-id-format";
        let archive_response = archive_post(server, invalid_post_id).await;

        archive_response.assert_status_failure();
    }
);

test_with_server!(
    unarchive_post_with_invalid_post_id_format,
    |server, ctx_state, config| {
        let (server, _user, _, _) = create_fake_login_test_user(&server).await;

        // Try to unarchive with malformed post ID
        let invalid_post_id = "invalid-post-id-format";
        let unarchive_response = unarchive_post(server, invalid_post_id).await;

        unarchive_response.assert_status_failure();
    }
);
