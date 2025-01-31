#[cfg(test)]
mod tests {
    use axum_test::multipart::MultipartForm;
    use surrealdb::sql::Thing;
    use uuid::Uuid;

    use crate::test_utils::{create_login_test_user, create_test_server};
    use sb_community::entity::community_entitiy::CommunityDbService;
    use sb_community::entity::discussion_entitiy::{Discussion, DiscussionDbService};
    use sb_community::entity::post_entitiy::PostDbService;
    use sb_community::routes::community_routes::CommunityInput;
    use sb_community::routes::discussion_routes::{DiscussionInput, DiscussionPostView};
    use sb_community::routes::discussion_topic_routes::TopicInput;
    use sb_middleware::ctx::Ctx;
    use sb_middleware::error::CtxResult;
    use sb_middleware::utils::db_utils::IdentIdName;
    use sb_middleware::utils::extractor_utils::DiscussionParams;
    use sb_middleware::utils::request_utils::CreatedResponse;
    use sb_middleware::utils::string_utils::get_string_thing;
    use sb_user_auth::entity::access_right_entity::AccessRightDbService;
    use sb_user_auth::entity::authorization_entity::{Authorization, AUTH_ACTIVITY_OWNER};
    use sb_user_auth::entity::local_user_entity::LocalUserDbService;

    #[tokio::test]
    async fn get_discussion_view() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;
        let disc_name = "discName1".to_lowercase();

        let create_response = server
            .post("/api/community")
            .json(&CommunityInput {
                id: "".to_string(),
                name_uri: disc_name.clone(),
                title: "The Community Test".to_string(),
            })
            .await;
        let created = &create_response.json::<CreatedResponse>();
        // dbg!(&created);

        let comm_id = Thing::try_from(created.id.clone()).unwrap();
        let comm_name = created.uri.clone().unwrap();

        &create_response.assert_status_success();

        let create_response = server
            .post("/api/discussion")
            .json(&DiscussionInput {
                id: "".to_string(),
                community_id: comm_id.to_raw(),
                title: "The Discussion".to_string(),
                image_uri: None,
            })
            .await;
        let created = &create_response.json::<CreatedResponse>();
        // dbg!(&created);

        let disc_id = Thing::try_from(created.id.clone()).unwrap();
        // let disc_name = created.uri.clone();

        &create_response.assert_status_success();

        let topic_resp = server
            .post(format!("/api/discussion/{}/topic", created.id.clone()).as_str())
            .json(&TopicInput {
                id: "".to_string(),
                title: "topic1".to_string(),
                hidden: None,
                access_rule_id: "".to_string(),
            })
            .await;
        dbg!(&topic_resp);
        &topic_resp.assert_status_success();

        let disc_rec = DiscussionDbService {
            db: &ctx_state._db,
            ctx: &Ctx::new(Ok(user_ident), Uuid::new_v4(), false),
        }
        .get(IdentIdName::Id(disc_id.clone()))
        .await;
        assert_eq!(disc_rec.clone().unwrap().topics.unwrap().len(), 1);
        let topics = disc_rec.unwrap().topics.unwrap();
        let topic_id = topics[0].clone();

        let post_name = "post title Name 1".to_string();
        let create_post = server
            .post(format!("/api/discussion/{disc_id}/post").as_str())
            .multipart(
                MultipartForm::new()
                    .add_text("title", post_name.clone())
                    .add_text("content", "contentttt111")
                    .add_text("topic_id", ""),
            )
            .await;
        &create_post.assert_status_success();

        let post_name2 = "post title Name 2?&$^%! <>end".to_string();
        let create_response2 = server
            .post(format!("/api/discussion/{disc_id}/post").as_str())
            .multipart(
                MultipartForm::new()
                    .add_text("title", post_name2.clone())
                    .add_text("content", "contentttt222")
                    .add_text("topic_id", topic_id.clone().to_raw()),
            )
            .await;
        &create_response2.assert_status_success();

        let disc_posts = PostDbService {
            db: &ctx_state._db,
            ctx: &Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false),
        }
        .get_by_discussion_desc_view::<DiscussionPostView>(
            disc_id.clone(),
            DiscussionParams {
                topic_id: None,
                start: None,
                count: None,
            },
        )
        .await;
        let disc_posts_top1 = PostDbService {
            db: &ctx_state._db,
            ctx: &Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false),
        }
        .get_by_discussion_desc_view::<DiscussionPostView>(
            disc_id,
            DiscussionParams {
                topic_id: Some(topic_id),
                start: None,
                count: None,
            },
        )
        .await;
        assert_eq!(disc_posts.is_ok(), true);
        assert_eq!(disc_posts.unwrap().len(), 2);
        assert_eq!(disc_posts_top1.is_ok(), true);
        assert_eq!(disc_posts_top1.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn create_discussion() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;
        let disc_name = "discName1".to_lowercase();
        let disc_name2 = "discName2".to_lowercase();

        let create_response = server
            .post("/api/community")
            .json(&CommunityInput {
                id: "".to_string(),
                name_uri: disc_name.clone(),
                title: "The Community Test".to_string(),
            })
            .await;
        let created = &create_response.json::<CreatedResponse>();
        // dbg!(&created);

        let comm_id = Thing::try_from(created.id.clone()).unwrap();
        let comm_name = created.uri.clone().unwrap();

        &create_response.assert_status_success();

        let create_response = server
            .post("/api/discussion")
            .json(&DiscussionInput {
                id: "".to_string(),
                community_id: comm_id.to_raw(),
                title: "The Discussion".to_string(),
                image_uri: None,
            })
            .await;
        let created = &create_response.json::<CreatedResponse>();
        // dbg!(&created);
        let disc_name = created.uri.clone();
        assert_eq!(disc_name, None);
        let disc_id = created.id.clone();

        &create_response.assert_status_success();

        // same username should return error
        // let create_response2 = server.post("/api/discussion").json(&DiscussionInput { id: None, community_id: comm_id.to_raw(), title: "The Discussion2".to_string(), topics: None }).await;

        // dbg!(&create_response2);
        // &create_response2.assert_status_bad_request();

        // let create_response2 = server.post("/api/discussion").json(&DiscussionInput { id: None, community_id: comm_id.to_raw(), title: "The Discussion22".to_string(), topics: None }).await;
        // &create_response2.assert_status_success();
        // let created2 = &create_response2.json::<CreatedResponse>();
        // let disc2_id = created2.id.clone();

        let ctx = &Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);
        let comm_db = CommunityDbService {
            db: &ctx_state._db,
            ctx: &ctx,
        };
        let comm = comm_db.get(IdentIdName::Id(comm_id.clone())).await;
        let comm_disc_id = comm.unwrap().profile_discussion.unwrap();

        let disc_db = DiscussionDbService {
            db: &ctx_state._db,
            ctx: &ctx,
        };

        let disc = disc_db
            .get(IdentIdName::Id(get_string_thing(created.id.clone()).expect("thing")).into())
            .await;
        let comm_disc: CtxResult<Discussion> = disc_db.get(IdentIdName::Id(comm_disc_id)).await;
        assert_eq!(comm_disc.unwrap().belongs_to.eq(&comm_id.clone()), true);
        // let disc_by_uri = disc_db.get(IdentIdName::ColumnIdent { column: "name_uri".to_string(), val: disc_name.to_string(), rec: false}).await;
        let discussion = disc.unwrap();
        // let discussion_by_uri = disc_by_uri.unwrap();
        assert_eq!(discussion.clone().topics, None);
        // assert_eq!(discussion.clone().name_uri.unwrap(), disc_name.clone());
        // assert_eq!(discussion_by_uri.clone().name_uri.unwrap(), disc_name.clone());

        let db_service = LocalUserDbService {
            db: &ctx_state._db,
            ctx: &ctx,
        };
        let aright_db_service = AccessRightDbService {
            db: &ctx_state._db,
            ctx: &ctx,
        };
        let user = db_service
            .get(IdentIdName::Id(get_string_thing(user_ident.clone()).expect("thing")).into())
            .await;
        let user = user.unwrap();

        let smaller_auth = Authorization {
            authorize_record_id: discussion.clone().id.unwrap(),
            authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
            authorize_height: 98,
        };
        let higher_auth = Authorization {
            authorize_record_id: discussion.id.unwrap(),
            authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
            authorize_height: 100,
        };

        assert_eq!(
            smaller_auth
                .ge(&higher_auth, ctx, &ctx_state._db)
                .await
                .is_err(),
            false
        );

        let mut found: Vec<Authorization> = vec![];
        let user_auth = aright_db_service
            .get_authorizations(&Thing::try_from(user_ident).unwrap())
            .await
            .expect("user authorizations");
        for v in user_auth.clone() {
            let is_ge = v.ge(&smaller_auth, ctx, &ctx_state._db).await;
            if is_ge.is_ok() {
                found.push(v);
            }
        }
        assert_eq!(found.len(), 1);

        let mut found: Vec<Authorization> = vec![];
        for v in user_auth.clone() {
            let is_ge = v.ge(&higher_auth, ctx, &ctx_state._db).await;
            if is_ge.unwrap() {
                found.push(v);
            }
        }
        assert_eq!(found.len(), 0);

        // let discussion_resp = server.get(format!("/discussion/{disc_name}").as_str()).add_header(HX_REQUEST, HeaderValue::from_static("true")).await;
        // let discussion_resp = server.get(format!("/discussion/{disc_id}").as_str()).add_header(HX_REQUEST, HeaderValue::from_static("true")).await;

        // dbg!(&discussion_resp);
        // &discussion_resp.assert_status_success();

        // let discussion_resp = server.get(format!("/discussion/{disc_id}?topic_id=discussion_topic:345").as_str()).add_header(HX_REQUEST, HeaderValue::from_static("true")).await;

        // dbg!(&discussion_resp);
        // &discussion_resp.assert_status_success();
    }
}
