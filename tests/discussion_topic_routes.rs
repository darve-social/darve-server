mod helpers;
use axum::extract::{Path, Query, State};
use axum_test::multipart::MultipartForm;
use community_entity::CommunityDbService;
use community_routes::CommunityInput;
use darve_server::entities::community::{community_entity, discussion_entity};
use darve_server::middleware;
use darve_server::routes::community::community_routes::get_community;
use darve_server::routes::community::{community_routes, discussion_topic_routes};
use discussion_entity::DiscussionDbService;
use discussion_topic_routes::{DiscussionTopicItemsEdit, TopicInput};
use middleware::ctx::Ctx;
use middleware::utils::db_utils::IdentIdName;
use middleware::utils::extractor_utils::DiscussionParams;
use middleware::utils::request_utils::CreatedResponse;
use surrealdb::sql::Thing;


use crate::helpers::create_fake_login_test_user;

test_with_server!(create_discussion, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();

    let comm_name = "community_123";
    let create_response = server
        .post("/api/community")
        .json(&CommunityInput {
            id: "".to_string(),
            name_uri: comm_name.to_string(),
            title: "The Community Test".to_string(),
        })
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<CreatedResponse>();
    // dbg!(&created);

    let comm_id = Thing::try_from(created.id.clone()).unwrap();

    create_response.assert_status_success();

    let ctx = Ctx::new(Ok(user_ident), false);
    let comm_db = CommunityDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };
    let comm = comm_db
        .get(IdentIdName::Id(comm_id.clone()))
        .await
        .expect("community struct");
    let comm_name = comm.name_uri.clone();
    let comm_disc_id = comm.default_discussion.unwrap();

    let disc_db = DiscussionDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };

    // let disc = disc_db.get(IdentIdName::Id(created.id.clone()).into()).await;
    let comm_disc = disc_db
        .get(IdentIdName::Id(comm_disc_id.clone()).into())
        .await;
    assert_eq!(
        comm_disc.clone().unwrap().belongs_to.eq(&comm_id.clone()),
        true
    );
    // let disc_by_uri = disc_db.get(IdentIdName::ColumnIdent { column: "name_uri".to_string(), val: disc_name.to_string(), rec: false}).await;
    let discussion = comm_disc.unwrap();
    // let discussion_by_uri = disc_by_uri.unwrap();
    assert_eq!(discussion.clone().topics, None);

    let topic_title = "topic1".to_string();
    let topic_resp = server
        .post(format!("/api/discussion/{}/topic", comm_disc_id).as_str())
        .json(&TopicInput {
            id: "".to_string(),
            title: topic_title.clone(),
            hidden: None,
            access_rule_id: "".to_string(),
        })
        .add_header("Accept", "application/json")
        .await;
    topic_resp.assert_status_success();
    let created = &topic_resp.json::<DiscussionTopicItemsEdit>();
    assert_eq!(&created.community_id, &comm_id);
    assert_eq!(&created.topics.get(0).unwrap().title, &topic_title);
    let topic1_id = created.topics.get(0).unwrap().id.clone();

    let post_name = "post title Name 1".to_string();
    let create_post = server
        .post(format!("/api/discussion/{comm_disc_id}/post").as_str())
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "contentttt111")
                .add_text("topic_id", ""),
        )
        .add_header("Accept", "application/json")
        .await;
    let created = create_post.json::<CreatedResponse>();
    create_post.assert_status_success();
    assert_eq!(created.id.len() > 0, true);

    let comm_view = get_community(
        State(ctx_state.clone()),
        ctx.clone(),
        Path(comm_name.clone()),
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
    assert_eq!(posts.len(), 1);

    let post_name = "post title Name 2".to_string();
    let create_post = server
        .post(format!("/api/discussion/{comm_disc_id}/post").as_str())
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "contentttt111")
                .add_text("topic_id", topic1_id.clone().unwrap().to_raw()),
        )
        .add_header("Accept", "application/json")
        .await;
    //.json(&PostInput { title: post_name, content: "contentttt".to_string(), topic_id: topic1_id.clone().unwrap().to_raw() }).await;
    let created = create_post.json::<CreatedResponse>();
    create_post.assert_status_success();
    assert_eq!(created.id.len() > 0, true);

    let comm_view = get_community(
        State(ctx_state.clone()),
        ctx.clone(),
        Path(comm_name.clone()),
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
    assert_eq!(posts.len(), 2);

    let comm_view = get_community(
        State(ctx_state.clone()),
        ctx,
        Path(comm_name),
        Query(DiscussionParams {
            topic_id: topic1_id,
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
    assert_eq!(posts.len(), 1);
});
