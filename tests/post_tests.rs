mod helpers;

use crate::helpers::create_fake_login_test_user;
use axum::extract::Query;
use axum::extract::{Path, State};
use axum_test::multipart::MultipartForm;
use community_entity::CommunityDbService;
use community_routes::get_community;
use darve_server::entities::community::community_entity;
use darve_server::entities::community::post_entity::Post;
use darve_server::entities::community::post_entity::PostDbService;
use darve_server::middleware::utils::db_utils::RecordWithId;
use darve_server::middleware::utils::string_utils::get_string_thing;
use darve_server::middleware::{self};
use darve_server::routes::community::community_routes;
use darve_server::routes::community::profile_routes::get_profile_community;
use darve_server::routes::posts::GetPostsQuery;
use helpers::community_helpers;
use helpers::community_helpers::create_fake_community;
use helpers::community_helpers::get_profile_discussion_id;
use helpers::post_helpers;
use helpers::post_helpers::get_posts;
use helpers::post_helpers::{
    create_fake_post, create_fake_post_with_file, create_fake_post_with_large_file,
};
use middleware::ctx::Ctx;
use middleware::utils::extractor_utils::DiscussionParams;

test_with_server!(create_post, |server, ctx_state, config| {
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
        .profile_discussion_view
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
            .add_text("content", "content")
            .add_text("topic_id", "");

        let response =
            helpers::post_helpers::create_post(server, &result.default_discussion, data).await;

        response.assert_status_success();

        let data_1 = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content")
            .add_text("topic_id", "");
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
