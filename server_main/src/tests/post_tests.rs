#[cfg(test)]
mod tests {

    use crate::test_utils::{
        create_fake_community, create_fake_post, create_login_test_user, create_test_server,
    };
    use axum::extract::{Path, State};
    use sb_community::entity::post_entitiy::PostDbService;
    use sb_community::routes::community_routes::get_community;
    use sb_middleware::ctx::Ctx;
    use sb_middleware::utils::extractor_utils::DiscussionParams;
    use surrealdb::sql::Thing;
    use uuid::Uuid;

    #[tokio::test]
    async fn create_post() {
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

        let result = create_fake_community(server, &ctx_state, user_ident.clone()).await;
        let _ = create_fake_post(server, &result.profile_discussion).await;
        let _ = create_fake_post(server, &result.profile_discussion).await;
        let _ = create_fake_post(server, &result.profile_discussion).await;
        let _ = create_fake_post(server, &result.profile_discussion).await;

        let ctx = Ctx::new(Ok(user_ident.clone()), Uuid::new_v4(), false);
        let post_db_service = PostDbService {
            db: &ctx_state._db,
            ctx: &ctx,
        };

        let user_thing_id = Thing::try_from(user_ident.clone()).unwrap();
        let result = post_db_service.get_latest(&user_thing_id, 2).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);

        let result = post_db_service.get_latest(&user_thing_id, 3).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 3);

        let result = post_db_service.get_latest(&user_thing_id, 1).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1)
    }
}
