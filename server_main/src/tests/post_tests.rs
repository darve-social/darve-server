#[cfg(test)]
mod tests {

    use crate::test_utils::{
        create_fake_community, create_fake_post, create_login_test_user, create_test_server,
    };
    use axum::extract::{Path, State};
    use sb_community::entity::community_entitiy::CommunityDbService;
    use sb_community::entity::post_entitiy::PostDbService;
    use sb_community::routes::community_routes::get_community;
    use sb_community::routes::profile_routes::get_profile_community;
    use sb_middleware::ctx::Ctx;
    use sb_middleware::db;
    use sb_middleware::error::CtxResult;
    use sb_middleware::utils::db_utils::RecordWithId;
    use sb_middleware::utils::extractor_utils::DiscussionParams;
    use sb_middleware::utils::string_utils::get_string_thing;
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
}
