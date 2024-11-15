
#[cfg(test)]
mod tests {
    use axum_test::multipart::MultipartForm;
    use surrealdb::sql::Thing;
    use uuid::Uuid;
    use sb_middleware::ctx::Ctx;
    use sb_community::entity::community_entitiy::{Community, CommunityDbService};
    use sb_community::routes::community_routes::CommunityInput;
    use sb_community::routes::reply_routes::PostReplyInput;
    use sb_middleware::utils::request_utils::CreatedResponse;
    use sb_user_auth::entity::authorization_entity::{get_parent_ids, get_same_level_record_ids, is_child_record};
    use crate::test_utils::{create_login_test_user, create_test_server};

    #[tokio::test]
    async fn create_reply() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;

        let create_response = server.post("/api/community").json(&CommunityInput { id: "".to_string(), create_custom_id: None, name_uri: "community-123".to_string(), title: "The Community Test".to_string() }).await;
        let created = &create_response.json::<CreatedResponse>();
        // dbg!(&created);

        let comm_id = Thing::try_from(created.id.clone()).unwrap();
        let comm_name = created.uri.clone().unwrap();
        &create_response.assert_status_success();
        let ctx = Ctx::new(Ok(user_ident), Uuid::new_v4(), false);
        let community_db_service = CommunityDbService { db: &ctx_state._db, ctx: &ctx.clone() };
        let community: Option<Community> = community_db_service.db.select((comm_id.clone().tb, comm_id.id.to_raw())).await.unwrap();

        // let commName = "comm-naMMe1".to_lowercase();
        // let create_response = server.post("/api/discussion").json(&DiscussionInput { id: None, community_id:comm_id.clone().to_raw(), discussion_uri: commName.clone(), title: "The Discussion".to_string(), topics: None }).await;
        // let created = &create_response.json::<CreatedResponse>();

        let comm_discussion_id = community.unwrap().main_discussion.unwrap().to_raw();
        assert_eq!(comm_discussion_id.len() > 0, true);

        let postName = "post title Name 1".to_string();
        let create_response = server.post(format!("/api/discussion/{comm_discussion_id}/post").as_str()).multipart(MultipartForm::new().add_text("title", postName.clone()).add_text("content", "contentttt111").add_text("topic_id", "")).await;
        &create_response.assert_status_success();
        let created = &create_response.json::<CreatedResponse>();
        dbg!(&created);

        let post_uri = &created.uri.clone().unwrap();

        let replyName = "post repl title Name 1".to_string();
        let create_response = server.post(format!("/api/discussion/{comm_discussion_id}/post/{post_uri}/reply").as_str()).json(&PostReplyInput { id: None, title: replyName, content: "contentttt".to_string() }).await;
        dbg!(&create_response);

        let replyName2 = "post repl Name 2?&$^%! <>end".to_string();
        let create_response2 = server.post(format!("/api/discussion/{comm_discussion_id}/post/{post_uri}/reply").as_str()).json(&PostReplyInput { id: None, title: replyName2.clone(), content: "contentttt222".to_string() }).await;

        let create_response3 = server.post(format!("/api/discussion/{comm_discussion_id}/post/{post_uri}/reply").as_str()).json(&PostReplyInput { id: None, title: replyName2.clone(), content: "contentttt33332".to_string() }).await;

        let create_response4 = server.post(format!("/api/discussion/{comm_discussion_id}/post/{post_uri}/reply").as_str()).json(&PostReplyInput { id: None, title: replyName2.clone(), content: "contentttt444442".to_string() }).await;
        // dbg!(&create_response);
        let created = &create_response.json::<CreatedResponse>();
        let created2 = &create_response2.json::<CreatedResponse>();
        let created3 = &create_response3.json::<CreatedResponse>();
        // dbg!(&created3);

        &create_response.assert_status_success();
        &create_response2.assert_status_success();
        &create_response3.assert_status_success();
        &create_response4.assert_status_success();

        let id1 = &Thing::try_from(comm_discussion_id.as_str()).unwrap();
        let id2 = &Thing::try_from(created2.id.as_str()).unwrap();
        let rids = get_same_level_record_ids(id1, id2, &ctx, &ctx_state._db).await.unwrap();
        // println!("id1={:?} id2={:?} //// parentIDs={:?}",id1,id2,rids);
        assert_eq!(id1.eq(&rids.1), true);
        assert_eq!(id2.eq(&rids.1), false);

        let rids = get_same_level_record_ids(id2, id1, &ctx, &ctx_state._db).await.unwrap();
        assert_eq!(id1.eq(&rids.1), true);
        assert_eq!(id2.eq(&rids.0), false);

        let is_child = is_child_record(id1, id2, &ctx, &ctx_state._db).await.unwrap();
        assert_eq!(is_child, true);

        let parents = get_parent_ids(id2, Some(&comm_id), &ctx, &ctx_state._db).await.unwrap();
        let parents1 = get_parent_ids(id2, None, &ctx, &ctx_state._db).await.unwrap();
        dbg!(&parents);

        assert_eq!(parents.first().unwrap().eq(&id2), true);
        assert_eq!(parents.last().unwrap().eq(&comm_id), true);
        assert_eq!(parents1.first().unwrap().eq(&id2), true);
        assert_eq!(parents1.last().unwrap().eq(&comm_id), true);

    }
}

