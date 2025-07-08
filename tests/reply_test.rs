mod helpers;
use crate::helpers::community_helpers::create_fake_community;
use crate::helpers::create_login_test_user;
use crate::helpers::post_helpers::create_fake_post;
use authorization_entity::{get_parent_ids, get_same_level_record_ids, is_child_record};
use community_entity::{Community, CommunityDbService};
use darve_server::middleware::utils::string_utils::get_string_thing;
use darve_server::{
    entities::{community::community_entity, user_auth::authorization_entity},
    middleware,
    routes::community::reply_routes,
};
use middleware::ctx::Ctx;
use middleware::utils::request_utils::CreatedResponse;
use reply_routes::PostReplyInput;
use surrealdb::sql::Thing;
use uuid::Uuid;

test_with_server!(create_reply, |server, ctx_state, config| {
    let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;

    let comm_id = get_string_thing(
        create_fake_community(server, &ctx_state, user_ident.clone())
            .await
            .id,
    )
    .unwrap();
    let ctx = Ctx::new(Ok(user_ident), Uuid::new_v4(), false);
    let community_db_service = CommunityDbService {
        db: &ctx_state.db.client,
        ctx: &ctx.clone(),
    };
    let community: Option<Community> = community_db_service
        .db
        .select((comm_id.clone().tb, comm_id.id.to_raw()))
        .await
        .unwrap();

    // let commName = "comm-naMMe1".to_lowercase();
    // let create_response = server.post("/api/discussion").json(&DiscussionInput { id: None, community_id:comm_id.clone().to_raw(), discussion_uri: commName.clone(), title: "The Discussion".to_string(), topics: None }).await;
    // let created = &create_response.json::<CreatedResponse>();

    let comm_disc_thing = community.unwrap().default_discussion.unwrap();
    let comm_discussion_id = comm_disc_thing.to_raw();
    assert_eq!(comm_discussion_id.len() > 0, true);

    let created_post = create_fake_post(server, &comm_disc_thing, None, None).await;

    let post_uri = &created_post.uri.clone();

    let reply_name = "post repl title Name 1".to_string();
    let create_response = server
        .post(format!("/api/discussion/{comm_discussion_id}/post/{post_uri}/reply").as_str())
        .json(&PostReplyInput {
            id: None,
            title: reply_name,
            content: "contentttt".to_string(),
        })
        .add_header("Accept", "application/json")
        .await;
    dbg!(&create_response);

    let reply_name2 = "post repl Name 2?&$^%! <>end".to_string();
    let create_response2 = server
        .post(format!("/api/discussion/{comm_discussion_id}/post/{post_uri}/reply").as_str())
        .json(&PostReplyInput {
            id: None,
            title: reply_name2.clone(),
            content: "contentttt222".to_string(),
        })
        .add_header("Accept", "application/json")
        .await;

    let create_response3 = server
        .post(format!("/api/discussion/{comm_discussion_id}/post/{post_uri}/reply").as_str())
        .json(&PostReplyInput {
            id: None,
            title: reply_name2.clone(),
            content: "contentttt33332".to_string(),
        })
        .add_header("Accept", "application/json")
        .await;

    let create_response4 = server
        .post(format!("/api/discussion/{comm_discussion_id}/post/{post_uri}/reply").as_str())
        .json(&PostReplyInput {
            id: None,
            title: reply_name2.clone(),
            content: "contentttt444442".to_string(),
        })
        .add_header("Accept", "application/json")
        .await;
    let _ = create_response.json::<CreatedResponse>();
    let created2 = &create_response2.json::<CreatedResponse>();
    let _ = create_response3.json::<CreatedResponse>();

    create_response.assert_status_success();
    create_response2.assert_status_success();
    create_response3.assert_status_success();
    create_response4.assert_status_success();

    let id1 = &Thing::try_from(comm_discussion_id.as_str()).unwrap();
    let id2 = &Thing::try_from(created2.id.as_str()).unwrap();
    let rids = get_same_level_record_ids(id1, id2, &ctx, &ctx_state.db.client)
        .await
        .unwrap();
    // println!("id1={:?} id2={:?} //// parentIDs={:?}",id1,id2,rids);
    assert_eq!(id1.eq(&rids.1), true);
    assert_eq!(id2.eq(&rids.1), false);

    let rids = get_same_level_record_ids(id2, id1, &ctx, &ctx_state.db.client)
        .await
        .unwrap();
    assert_eq!(id1.eq(&rids.1), true);
    assert_eq!(id2.eq(&rids.0), false);

    let is_child = is_child_record(id1, id2, &ctx, &ctx_state.db.client)
        .await
        .unwrap();
    assert_eq!(is_child, true);

    let parents = get_parent_ids(id2, Some(&comm_id), &ctx, &ctx_state.db.client)
        .await
        .unwrap();
    let parents1 = get_parent_ids(id2, None, &ctx, &ctx_state.db.client)
        .await
        .unwrap();
    dbg!(&parents);

    assert_eq!(parents.first().unwrap().eq(&id2), true);
    assert_eq!(parents.last().unwrap().eq(&comm_id), true);
    assert_eq!(parents1.first().unwrap().eq(&id2), true);
    assert_eq!(parents1.last().unwrap().eq(&comm_id), true);
});
