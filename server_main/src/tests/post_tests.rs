
#[cfg(test)]
mod tests {
    use axum::extract::{Path, State};
    use axum_test::multipart::MultipartForm;
    use surrealdb::sql::Thing;
    use uuid::Uuid;

    use crate::test_utils::{create_login_test_user, create_test_server};
    use sb_community::entity::community_entitiy::{Community, CommunityDbService};
    use sb_community::routes::community_routes::{get_community, CommunityInput};
    use sb_middleware::ctx::Ctx;
    use sb_middleware::utils::extractor_utils::DiscussionParams;
    use sb_middleware::utils::request_utils::CreatedResponse;

    #[tokio::test]
    async fn create_post() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;

        let comm_name = "comm-naMMe1".to_lowercase();

        let create_response = server.post("/api/community").json(&CommunityInput { id: "".to_string(), create_custom_id: None, name_uri: comm_name.clone(), title: "The Community Test".to_string() }).await;
        let created = &create_response.json::<CreatedResponse>();

        let comm_id = Thing::try_from(created.id.clone()).unwrap();
        let comm_name = created.uri.clone().unwrap();
        &create_response.assert_status_success();

        let ctx = Ctx::new(Ok(user_ident), Uuid::new_v4(), false);
        let community_db_service = CommunityDbService { db: &ctx_state._db, ctx: &ctx };
        let community:Community = community_db_service.db.select((&comm_id.tb, comm_id.id.to_raw())).await.unwrap().unwrap();
        assert_eq!(comm_name, community.name_uri.clone());
        let community_discussion_id = community.profile_discussion.clone().unwrap();

        let post_name = "post title Name 1".to_string();
        let create_post = server.post(format!("/api/discussion/{community_discussion_id}/post").as_str()).multipart(MultipartForm::new().add_text("title", post_name.clone()).add_text("content", "contentttt").add_text("topic_id", "")).await;
        let created = create_post.json::<CreatedResponse>();
        &create_post.assert_status_success();
        assert_eq!(created.id.len() > 0, true);

        let post_name2 = "post title Name 2?&$^%! <>end".to_string();
        let create_response2 = server.post(format!("/api/discussion/{community_discussion_id}/post").as_str()).multipart(MultipartForm::new().add_text("title", post_name2.clone()).add_text("content", "contentttt222").add_text("topic_id", "")).await;

        let create_response4 = server.post(format!("/api/discussion/{community_discussion_id}/post").as_str()).multipart(MultipartForm::new().add_text("title", post_name2.clone()).add_text("content", "contentttt444442").add_text("topic_id", "")).await;
        let created = &create_response.json::<CreatedResponse>();
        let created2 = &create_response2.json::<CreatedResponse>();

        &create_response2.assert_status_success();
        // can't have same title
        &create_response4.assert_status_bad_request();

        let comm_posts_response = server.get(format!("/api/discussion/{community_discussion_id}/post").as_str()).await;

        let comm_view = get_community(State(ctx_state), ctx, Path(comm_name), DiscussionParams{
            topic_id: None,
            start: None,
            count: None,
        }).await.expect("community page");
        let posts = comm_view.community_view.unwrap().profile_discussion_view.unwrap().posts;
        assert_eq!(posts.len(), 2);
    }

}

