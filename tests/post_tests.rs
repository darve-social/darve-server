mod helpers;

use crate::helpers::create_fake_login_test_user;
use crate::helpers::post_helpers::create_post;
use axum_test::multipart::MultipartForm;
use darve_server::entities::community::discussion_entity::DiscussionDbService;
use darve_server::entities::community::post_entity::Post;
use darve_server::entities::community::post_entity::PostDbService;
use darve_server::interfaces::repositories::tags::TagsRepositoryInterface;
use darve_server::middleware::utils::db_utils::Pagination;
use darve_server::middleware::utils::db_utils::RecordWithId;
use darve_server::middleware::utils::string_utils::get_string_thing;
use darve_server::middleware::{self};
use darve_server::routes::posts::GetPostsQuery;
use darve_server::services::post_service::PostView;
use helpers::community_helpers;
use helpers::community_helpers::get_profile_discussion_id;
use helpers::post_helpers::get_posts;
use helpers::post_helpers::{
    create_fake_post, create_fake_post_with_file, create_fake_post_with_large_file,
};
use middleware::ctx::Ctx;
use middleware::utils::extractor_utils::DiscussionParams;

test_with_server!(create_post_test, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;

    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();

    assert_eq!(posts.len(), 4);
});

test_with_server!(
    create_post_with_the_same_name,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let user_ident = user.id.as_ref().unwrap().to_raw();

        let result =
            DiscussionDbService::get_profile_discussion_id(&get_string_thing(user_ident).unwrap());

        let title = "TEST_TEST";
        let data = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content");

        let response = helpers::post_helpers::create_post(server, &result, data).await;

        response.assert_status_success();

        let data_1 = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content");
        let response_1 = helpers::post_helpers::create_post(server, &result, data_1).await;

        response_1.assert_status_success();
    }
);

test_with_server!(create_post_with_file_test, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();

    let result = get_profile_discussion_id(server, user_ident.clone()).await;
    let _ = create_fake_post_with_large_file(server, &ctx_state, &result).await;
    let _ = create_fake_post_with_file(server, &ctx_state, &result).await;

    let posts = server
        .get(&format!("/api/discussions/{}/posts", result.to_raw()))
        .await
        .json::<Vec<PostView>>();

    let post = posts.last().unwrap();
    assert_eq!(post.media_links.as_ref().unwrap().len(), 1);
    assert!(post.media_links.as_ref().unwrap()[0].contains("test_image_2mb.jpg"));
});

test_with_server!(get_latest, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();
    let ctx = Ctx::new(Ok(user_ident.clone()), false);
    let user_thing_id = get_string_thing(user_ident).unwrap();

    let default_discussion = DiscussionDbService::get_profile_discussion_id(&user_thing_id);
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;

    let result = PostDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .get_by_discussion_desc_view::<RecordWithId>(
        default_discussion.clone(),
        DiscussionParams {
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
        default_discussion.clone(),
        DiscussionParams {
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
        default_discussion.clone(),
        DiscussionParams {
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
        "tag0".to_string(),
        "tag1".to_string(),
        "tag2".to_string(),
        "tag3".to_string(),
        "tag4".to_string(),
        "tag5".to_string(),
        "tag6".to_string(),
    ];
    let _ = create_fake_post(
        server,
        &default_discussion,
        None,
        Some(Vec::from(&tags[0..5])),
    )
    .await;

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();
    assert_eq!(posts.len(), 2);

    let tags = ctx_state
        .db
        .tags
        .get(
            None,
            Pagination {
                order_by: None,
                order_dir: None,
                count: 10,
                start: 0,
            },
        )
        .await
        .unwrap();
    assert!(tags.contains(&tags[0]));
    assert!(tags.contains(&tags[1]));
    assert!(tags.contains(&tags[2]));
    assert!(tags.contains(&tags[3]));
    assert!(tags.contains(&tags[4]));
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

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();
    assert_eq!(posts.len(), 4);

    let posts_res = get_posts(
        &server,
        GetPostsQuery {
            tag: tags[0].clone(),
            start: None,
            count: None,
            order_dir: None,
        },
    )
    .await;

    posts_res.assert_status_success();
    let posts = posts_res.json::<Vec<Post>>();
    assert_eq!(posts.len(), 2);

    let posts_res = get_posts(
        &server,
        GetPostsQuery {
            tag: tags[1].clone(),
            start: None,
            count: None,
            order_dir: None,
        },
    )
    .await;

    posts_res.assert_status_success();
    let posts = posts_res.json::<Vec<Post>>();
    assert_eq!(posts.len(), 1);

    let posts_res = get_posts(
        &server,
        GetPostsQuery {
            tag: "rust".to_string(),
            start: None,
            count: None,
            order_dir: None,
        },
    )
    .await;

    posts_res.assert_status_success();
    let posts = posts_res.json::<Vec<Post>>();
    assert_eq!(posts.len(), 0);

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();
    assert_eq!(posts.len(), 4);
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
