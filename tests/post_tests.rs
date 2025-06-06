mod helpers;

use crate::helpers::create_fake_login_test_user;
use crate::helpers::create_test_server;
use axum::extract::{Path, State};
use axum_test::multipart::MultipartForm;
use community_entity::CommunityDbService;
use community_routes::get_community;
use darve_server::entities::community::community_entity;
use darve_server::entities::community::post_entity::Post;
use darve_server::entities::community::post_entity::PostDbService;
use darve_server::middleware::error::CtxResult;
use darve_server::middleware::utils::db_utils::RecordWithId;
use darve_server::middleware::utils::string_utils::get_string_thing;
use darve_server::middleware::{self, db};
use darve_server::routes::community::community_routes;
use darve_server::routes::community::post_routes::GetPostsQuery;
use darve_server::routes::community::profile_routes::get_profile_community;
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
use serde_json::{from_value, Value};
use surrealdb::sql::Thing;
use uuid::Uuid;

#[tokio::test]
async fn create_post() {
    let (server, ctx_state) = create_test_server().await;
    let (server, user, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();

    let result = create_fake_community(server, &ctx_state, user_ident.clone()).await;

    let ctx = Ctx::new(Ok(user_ident), Uuid::new_v4(), false);

    let _ = create_fake_post(server, &result.default_discussion, None, None).await;
    let _ = create_fake_post(server, &result.default_discussion, None, None).await;
    let _ = create_fake_post(server, &result.default_discussion, None, None).await;
    let _ = create_fake_post(server, &result.default_discussion, None, None).await;

    let comm_view = get_community(
        State(ctx_state.clone()),
        ctx,
        Path(result.name),
        DiscussionParams {
            topic_id: None,
            start: None,
            count: None,
        },
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
}

#[tokio::test]
async fn create_post_with_the_same_name() {
    let (server, ctx_state) = create_test_server().await;
    let (server, user, _) = create_fake_login_test_user(&server).await;
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

#[tokio::test]
async fn create_post_with_file_test() {
    let (server, ctx_state) = create_test_server().await;
    let (server, user, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();

    let result = get_profile_discussion_id(server, user_ident.clone()).await;
    let _ = create_fake_post_with_large_file(server, &ctx_state, &result).await;
    let _ = create_fake_post_with_file(server, &ctx_state, &result).await;

    let posts_res = get_posts(&server, None).await;
    posts_res.assert_status_success();
    let posts_value = posts_res.json::<Value>();
    let posts: Vec<Post> = from_value(posts_value.get("posts").unwrap().to_owned()).unwrap();
    let post = posts.last().unwrap();
    assert_eq!(post.media_links.as_ref().unwrap().len(), 1);
    assert!(post.media_links.as_ref().unwrap()[0].contains("test_image_2mb.jpg"));
}

#[tokio::test]
async fn get_latest() {
    let (server, ctx_state) = create_test_server().await;
    let (server, user, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();
    let ctx = Ctx::new(Ok(user_ident.clone()), Uuid::new_v4(), false);
    let user_thing_id = get_string_thing(user_ident).unwrap();

    let default_discussion = get_profile_community(&ctx_state._db, &ctx, user_thing_id.clone())
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
        db: &ctx_state._db,
    }
    .get_profile_community(user_thing_id)
    .await;
    let discussion_id = profile_comm.unwrap().default_discussion.unwrap();
    let result = get_latest_posts(2, discussion_id.clone(), &ctx, &ctx_state._db).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 2);

    let result = get_latest_posts(3, discussion_id.clone(), &ctx, &ctx_state._db).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 3);

    let result = get_latest_posts(1, discussion_id, &ctx, &ctx_state._db).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1)
}

#[tokio::test]
async fn create_post_with_tags() {
    let (server, _) = create_test_server().await;
    let (server, user, _) = create_fake_login_test_user(&server).await;
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
    let posts_value = posts_res.json::<Value>();
    let posts: Vec<Post> = from_value(posts_value.get("posts").unwrap().to_owned()).unwrap();
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0].tags.as_ref().unwrap()[0], tags[0]);
    assert_eq!(posts[0].tags.as_ref().unwrap()[1], tags[1]);
    assert_eq!(posts[1].tags, None);
    let data = post_helpers::build_fake_post(None, Some(tags.clone()));
    let response = post_helpers::create_post(server, &default_discussion, data).await;
    response.assert_status_unprocessable_entity();
}

#[tokio::test]
async fn filter_posts_by_tag() {
    let (server, _) = create_test_server().await;
    let (server, user, _) = create_fake_login_test_user(&server).await;
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
    let posts_value = posts_res.json::<Value>();
    let posts: Vec<Post> = from_value(posts_value.get("posts").unwrap().to_owned()).unwrap();
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
    let posts_value = posts_res.json::<Value>();
    let posts: Vec<Post> = from_value(posts_value.get("posts").unwrap().to_owned()).unwrap();
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
    let posts_value = posts_res.json::<Value>();
    let posts: Vec<Post> = from_value(posts_value.get("posts").unwrap().to_owned()).unwrap();
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
    let posts_value = posts_res.json::<Value>();
    let posts: Vec<Post> = from_value(posts_value.get("posts").unwrap().to_owned()).unwrap();
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
    let posts_value = posts_res.json::<Value>();
    let posts: Vec<Post> = from_value(posts_value.get("posts").unwrap().to_owned()).unwrap();
    assert_eq!(posts.len(), 1);
}

async fn get_latest_posts(
    posts_nr: i8,
    profile_discussion_id: Thing,
    ctx: &Ctx,
    db: &db::Db,
) -> CtxResult<Vec<RecordWithId>> {
    PostDbService { db, ctx }
        .get_by_discussion_desc_view::<RecordWithId>(
            profile_discussion_id,
            DiscussionParams {
                topic_id: None,
                start: Some(0),
                count: Some(posts_nr),
            },
        )
        .await
}
