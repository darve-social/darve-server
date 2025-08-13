mod helpers;
use crate::helpers::create_fake_login_test_user;
use crate::helpers::post_helpers::create_fake_post;
use authorization_entity::{get_parent_ids, get_same_level_record_ids, is_child_record};
use community_entity::CommunityDbService;
use darve_server::entities::community::discussion_entity::Discussion;
use darve_server::entities::{community::community_entity, user_auth::authorization_entity};
use darve_server::middleware::ctx::Ctx;
use darve_server::models::view::reply::ReplyView;
use darve_server::services::discussion_service::CreateDiscussion;
use serde_json::json;
use surrealdb::sql::Thing;

test_with_server!(create_reply, |server, ctx_state, config| {
    let (server, user_ident, _, _) = create_fake_login_test_user(&server).await;
    let ctx = Ctx::new(Ok(user_ident.id.as_ref().unwrap().to_raw()), false);
    let comm_id = CommunityDbService::get_profile_community_id(&user_ident.id.as_ref().unwrap());
    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            title: "The Community Test".to_string(),
            community_id: comm_id.to_raw(),
            image_uri: None,
            chat_user_ids: None,
            private_discussion_users_final: false,
        })
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_success();
    let created = &create_response.json::<Discussion>();

    let comm_disc_thing = created.id.as_ref().unwrap().clone();
    let comm_discussion_id = comm_disc_thing.to_raw();
    assert_eq!(comm_discussion_id.len() > 0, true);

    let created_post = create_fake_post(server, &comm_disc_thing, None, None).await;

    let post_uri = &created_post.uri.clone();

    let create_response = server
        .post(format!("/api/posts/{post_uri}/replies").as_str())
        .json(&json!({
            "content": "contentttt222".to_string(),
        }))
        .add_header("Accept", "application/json")
        .await;
    dbg!(&create_response);

    let create_response2 = server
        .post(format!("/api/posts/{post_uri}/replies").as_str())
        .json(&json!({
            "content": "contentttt222".to_string(),
        }))
        .add_header("Accept", "application/json")
        .await;

    let create_response3 = server
        .post(format!("/api/posts/{post_uri}/replies").as_str())
        .json(&json!({
            "content": "contentttt222".to_string(),
        }))
        .add_header("Accept", "application/json")
        .await;

    let create_response4 = server
        .post(format!("/api/posts/{post_uri}/replies").as_str())
        .json(&json!({
            "content": "contentttt222".to_string(),
        }))
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_success();
    create_response2.assert_status_success();
    create_response3.assert_status_success();
    create_response4.assert_status_success();

    let _ = create_response.json::<ReplyView>();
    let created2 = &create_response2.json::<ReplyView>();
    let _ = create_response3.json::<ReplyView>();

    let id1 = &Thing::try_from(comm_discussion_id.as_str()).unwrap();
    let id2 = &created2.id;
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
