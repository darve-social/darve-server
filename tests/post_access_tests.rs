mod helpers;

use crate::helpers::create_fake_login_test_user;
use crate::helpers::post_helpers::create_post;
use axum_test::multipart::MultipartForm;
use darve_server::entities::community::community_entity::CommunityDbService;
use darve_server::entities::community::discussion_entity::Discussion;
use darve_server::entities::community::discussion_entity::DiscussionDbService;
// use darve_server::entities::community::post_entity::Post;
use darve_server::entities::community::post_entity::PostType;
use darve_server::models::view::post::PostView;
use darve_server::models::web::UserView;
use fake::faker;
use fake::Fake;
use helpers::post_helpers::create_fake_post;
use serde_json::json;

test_with_server!(
    try_to_add_users_to_public_post_test,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;

        let default_discussion =
            DiscussionDbService::get_profile_discussion_id(&user2.id.as_ref().unwrap());

        let post = create_fake_post(server, &default_discussion, None, None).await;

        let response = server
            .post(&format!("/api/posts/{}/add_users", post.id))
            .json(&json!({ "user_ids": [user.id.as_ref().unwrap().to_raw()] }))
            .await;

        response.assert_status_forbidden();
    }
);

test_with_server!(
    try_to_add_users_to_idea_post_test,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;

        let default_discussion =
            DiscussionDbService::get_profile_discussion_id(&user2.id.as_ref().unwrap());

        let title = faker::lorem::en::Sentence(7..20).fake::<String>();
        let data = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content")
            .add_text("is_idea", true);

        let post = create_post(server, &default_discussion, data).await;
        post.assert_status_ok();
        let post = post.json::<PostView>();
        assert_eq!(post.r#type, PostType::Idea);
        let response = server
            .post(&format!("/api/posts/{}/add_users", post.id.to_raw()))
            .json(&json!({ "user_ids": [user.id.as_ref().unwrap().to_raw()] }))
            .await;

        response.assert_status_forbidden();
    }
);
test_with_server!(add_users_to_post_test, |server, ctx_state, config| {
    let (server, user0, _, _) = create_fake_login_test_user(&server).await;
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let (server, user2, _, _) = create_fake_login_test_user(&server).await;

    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user2.id.as_ref().unwrap());

    let title = faker::lorem::en::Sentence(7..20).fake::<String>();
    let data = MultipartForm::new()
        .add_text("title", title)
        .add_text("content", "content")
        .add_text("users", user.id.as_ref().unwrap().to_raw());

    let post = create_post(server, &default_discussion, data).await;
    post.assert_status_ok();
    let post = post.json::<PostView>();
    assert_eq!(post.r#type, PostType::Private);

    let response = server
        .post(&format!("/api/posts/{}/add_users", post.id.to_raw()))
        .json(&json!({ "user_ids": [user0.id.as_ref().unwrap().to_raw()] }))
        .await;

    response.assert_status_ok();

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();

    assert_eq!(posts.len(), 1);
    let post = &posts[0];
    assert_eq!(post.users.as_ref().unwrap().len(), 3);
});

test_with_server!(remove_users_from_post_test, |server, ctx_state, config| {
    let (server, user0, _, _) = create_fake_login_test_user(&server).await;
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let (server, user2, _, _) = create_fake_login_test_user(&server).await;

    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user2.id.as_ref().unwrap());

    let title = faker::lorem::en::Sentence(7..20).fake::<String>();
    let data = MultipartForm::new()
        .add_text("title", title)
        .add_text("content", "content")
        .add_text("users", user.id.as_ref().unwrap().to_raw());

    let post = create_post(server, &default_discussion, data).await;
    post.assert_status_ok();
    let post = post.json::<PostView>();
    assert_eq!(post.r#type, PostType::Private);

    let response = server
        .post(&format!("/api/posts/{}/add_users", post.id.to_raw()))
        .json(&json!({ "user_ids": [user0.id.as_ref().unwrap().to_raw()] }))
        .await;

    response.assert_status_ok();

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();

    assert_eq!(posts.len(), 1);
    let post = &posts[0];
    assert_eq!(post.users.as_ref().unwrap().len(), 3);

    let response = server
        .post(&format!("/api/posts/{}/remove_users", post.id.to_raw()))
        .json(&json!({ "user_ids": [user0.id.as_ref().unwrap().to_raw()] }))
        .await;

    response.assert_status_ok();

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();

    assert_eq!(posts.len(), 1);
    let post = &posts[0];
    assert_eq!(post.users.as_ref().unwrap().len(), 2);
});

test_with_server!(
    try_to_remove_owner_from_post_test,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;

        let default_discussion =
            DiscussionDbService::get_profile_discussion_id(&user2.id.as_ref().unwrap());

        let title = faker::lorem::en::Sentence(7..20).fake::<String>();
        let data = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content")
            .add_text("users", user.id.as_ref().unwrap().to_raw());

        let post = create_post(server, &default_discussion, data).await;
        post.assert_status_ok();
        let post = post.json::<PostView>();
        assert_eq!(post.r#type, PostType::Private);

        let response = server
            .post(&format!("/api/posts/{}/remove_users", post.id.to_raw()))
            .json(&json!({ "user_ids": [user2.id.as_ref().unwrap().to_raw()] }))
            .await;

        response.assert_status_failure();
    }
);

test_with_server!(create_post_with_users_test, |server, ctx_state, config| {
    let (server, user2, _, _) = create_fake_login_test_user(&server).await;
    let (server, user, _, _) = create_fake_login_test_user(&server).await;

    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

    let title = "TEST_TEST";
    let data = MultipartForm::new()
        .add_text("title", title)
        .add_text("content", "content")
        .add_text("users", user.id.as_ref().unwrap().to_raw())
        .add_text("users", user2.id.as_ref().unwrap().to_raw());

    let response = create_post(server, &default_discussion, data).await;
    response.assert_status_success();
    let post = response.json::<PostView>();

    assert_eq!(post.r#type, PostType::Private);

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();

    assert_eq!(posts.len(), 1);

    let post = &posts[0];

    assert_eq!(post.users.as_ref().unwrap().len(), 2);
});

test_with_server!(
    create_post_with_users_omit_owner_test,
    |server, ctx_state, config| {
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let (server, user, _, _) = create_fake_login_test_user(&server).await;

        let default_discussion =
            DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

        let title = "TEST_TEST";
        let data = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content")
            .add_text("users", user2.id.as_ref().unwrap().to_raw());

        let response = create_post(server, &default_discussion, data).await;
        response.assert_status_success();
        let post = response.json::<PostView>();

        assert_eq!(post.r#type, PostType::Private);

        let posts = server
            .get(&format!(
                "/api/discussions/{}/posts",
                default_discussion.to_raw()
            ))
            .await
            .json::<Vec<PostView>>();

        assert_eq!(posts.len(), 1);

        let post = &posts[0];

        assert_eq!(post.users.as_ref().unwrap().len(), 2);
    }
);

test_with_server!(
    try_to_create_post_with_empty_users_test,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;

        let default_discussion =
            DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

        let title = "TEST_TEST";
        let data = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content")
            .add_text("users", "");

        let response = create_post(server, &default_discussion, data).await;
        response.assert_status_failure();
    }
);

test_with_server!(
    try_to_create_post_with_users_who_not_access_test,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let (server, user0, _, _) = create_fake_login_test_user(&server).await;
        let (server, user1, _, _) = create_fake_login_test_user(&server).await;
        let com_id = CommunityDbService::get_profile_community_id(user1.id.as_ref().unwrap());
        let title = faker::lorem::en::Sentence(7..20).fake::<String>();
        let discussion = server
            .post("/api/discussions")
            .json(&json!({"community_id": com_id.to_raw(), "title": title, "chat_user_ids" : [user0.id.as_ref().unwrap().to_raw()]}))
            .await
            .json::<Discussion>();

        let title = faker::lorem::en::Sentence(7..20).fake::<String>();
        let data = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content")
            .add_text("users", user.id.as_ref().unwrap());

        let response = create_post(server, &discussion.id, data).await;
        response.assert_status_forbidden();

        let title = faker::lorem::en::Sentence(7..20).fake::<String>();
        let data = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content")
            .add_text("users", user.id.as_ref().unwrap())
            .add_text("users", user0.id.as_ref().unwrap());

        let response = create_post(server, &discussion.id, data).await;
        response.assert_status_forbidden();
    }
);

test_with_server!(
    create_post_with_users_and_idea_test,
    |server, ctx_state, config| {
        let (server, user0, _, _) = create_fake_login_test_user(&server).await;
        let (server, user1, _, _) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());

        let title = faker::lorem::en::Sentence(7..20).fake::<String>();
        let data = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content")
            .add_text("is_idea", true)
            .add_text("users", user0.id.as_ref().unwrap());

        let response = create_post(server, &disc_id, data).await;
        response.assert_status_ok();

        let post = response.json::<PostView>();
        assert_eq!(post.r#type, PostType::Idea);
    }
);

test_with_server!(get_users_to_post_test, |server, ctx_state, config| {
    let (server, user0, _, _) = create_fake_login_test_user(&server).await;
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let (server, user1, _, _) = create_fake_login_test_user(&server).await;
    let (server, user2, _, _) = create_fake_login_test_user(&server).await;

    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user2.id.as_ref().unwrap());

    let title = faker::lorem::en::Sentence(7..20).fake::<String>();
    let data = MultipartForm::new()
        .add_text("title", title)
        .add_text("content", "content")
        .add_text("users", user.id.as_ref().unwrap().to_raw())
        .add_text("users", user0.id.as_ref().unwrap().to_raw())
        .add_text("users", user1.id.as_ref().unwrap().to_raw());

    let post = create_post(server, &default_discussion, data).await;
    post.assert_status_ok();
    let post = post.json::<PostView>();
    assert_eq!(post.r#type, PostType::Private);

    let response = server
        .get(&format!("/api/posts/{}/users", post.id.to_raw()))
        .await;

    response.assert_status_ok();

    let users = response.json::<Vec<UserView>>();
    assert_eq!(users.len(), 4);
});
