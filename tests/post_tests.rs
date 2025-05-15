mod helpers;

use crate::helpers::{create_login_test_user, create_test_server};
use axum::extract::{Path, State};
use axum_test::multipart::MultipartForm;
use community_entity::{Community, CommunityDbService};
use community_routes::{get_community, CommunityInput};
use darve_server::entities::community::community_entity;
use darve_server::entities::community::post_entity::PostDbService;
use darve_server::middleware::error::CtxResult;
use darve_server::middleware::utils::db_utils::RecordWithId;
use darve_server::middleware::utils::string_utils::get_string_thing;
use darve_server::middleware::{self, db};
use darve_server::routes::community::community_routes;
use darve_server::routes::community::profile_routes::get_profile_community;
use helpers::community_helpers::create_fake_community;
use helpers::post_helpers::create_fake_post;
use middleware::ctx::Ctx;
use middleware::utils::extractor_utils::DiscussionParams;
use middleware::utils::request_utils::CreatedResponse;
use surrealdb::sql::Thing;
use uuid::Uuid;

#[tokio::test]
async fn create_post() {
    let (server, ctx_state) = create_test_server().await;
    let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;

    let comm_name = "comm-naMMe1".to_lowercase();

    // create community
    let create_response = server
        .post("/api/community")
        .json(&CommunityInput {
            id: "".to_string(),
            name_uri: comm_name.clone(),
            title: "The Community Test".to_string(),
        })
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<CreatedResponse>();

    let comm_id = Thing::try_from(created.id.clone()).unwrap();
    let comm_name = created.uri.clone().unwrap();
    create_response.assert_status_success();

    let ctx = Ctx::new(Ok(user_ident), Uuid::new_v4(), false);
    let community_db_service = CommunityDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };
    let community: Community = community_db_service
        .db
        .select((&comm_id.tb, comm_id.id.to_raw()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(comm_name, community.name_uri.clone());
    let community_discussion_id = community.profile_discussion.clone().unwrap();

    let post_name = "post title Name 1".to_string();
    let create_post = server
        .post(format!("/api/discussion/{community_discussion_id}/post").as_str())
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "contentttt")
                .add_text("topic_id", ""),
        )
        .add_header("Accept", "application/json")
        .await;
    let created = create_post.json::<CreatedResponse>();
    create_post.assert_status_success();
    assert_eq!(created.id.len() > 0, true);

    let post_name2 = "post title Name 2?&$^%! <>end".to_string();
    let create_response2 = server
        .post(format!("/api/discussion/{community_discussion_id}/post").as_str())
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name2.clone())
                .add_text("content", "contentttt222")
                .add_text("topic_id", ""),
        )
        .add_header("Accept", "application/json")
        .await;

    let create_response4 = server
        .post(format!("/api/discussion/{community_discussion_id}/post").as_str())
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name2.clone())
                .add_text("content", "contentttt444442")
                .add_text("topic_id", ""),
        )
        .add_header("Accept", "application/json")
        .await;

    create_response2.assert_status_success();
    // can't have same title
    create_response4.assert_status_bad_request();

    let _ = server
        .get(format!("/api/discussion/{community_discussion_id}/post").as_str())
        .add_header("Accept", "application/json")
        .await;

    let comm_view = get_community(
        State(ctx_state.clone()),
        ctx,
        Path(comm_name),
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
    assert_eq!(posts.len(), 2);
}

#[tokio::test]
async fn create_post_test() {
    let (server, ctx_state) = create_test_server().await;
    let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;

    let result = create_fake_community(server, &ctx_state, user_ident.clone()).await;
    let _ = create_fake_post(server, &result.profile_discussion).await;
    let _ = create_fake_post(server, &result.profile_discussion).await;
    let _ = create_fake_post(server, &result.profile_discussion).await;
    let _ = create_fake_post(server, &result.profile_discussion).await;
    let ctx = Ctx::new(Ok(user_ident), Uuid::new_v4(), false);

    let comm_view = get_community(
        State(ctx_state.clone()),
        ctx,
        Path(result.name.clone()),
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
async fn get_latest() {
    let (server, ctx_state) = create_test_server().await;
    let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;
    let ctx = Ctx::new(Ok(user_ident.clone()), Uuid::new_v4(), false);
    let user_thing_id = get_string_thing(user_ident).unwrap();

    let profile_discussion = get_profile_community(&ctx_state._db, &ctx, user_thing_id.clone())
        .await
        .unwrap()
        .profile_discussion
        .unwrap();
    let _ = create_fake_post(server, &profile_discussion).await;
    let _ = create_fake_post(server, &profile_discussion).await;
    let _ = create_fake_post(server, &profile_discussion).await;
    let _ = create_fake_post(server, &profile_discussion).await;

    let profile_comm = CommunityDbService {
        ctx: &ctx,
        db: &ctx_state._db,
    }
    .get_profile_community(user_thing_id)
    .await;
    let discussion_id = profile_comm.unwrap().profile_discussion.unwrap();
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
