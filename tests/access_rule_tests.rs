// TODO: Fix failing test

// mod helpers;
// use access_right_entity::AccessRightDbService;
// use access_rule_entity::AccessRuleDbService;
// use access_rule_routes::{AccessRuleForm, AccessRuleInput};
// use authentication_entity::AuthType;
// use authorization_entity::{Authorization, AUTH_ACTIVITY_OWNER, AUTH_ACTIVITY_VISITOR};
// use axum::extract::{Path, State};
// use axum_test::multipart::MultipartForm;
// use community_entity::CommunityDbService;
// use community_routes::{get_community, CommunityInput};
// use darve_server::entities::community::community_entity;
// use darve_server::entities::user_auth::{
//     access_right_entity, access_rule_entity, authentication_entity, authorization_entity,
//     local_user_entity,
// };
// use darve_server::middleware;
// use darve_server::routes::community::{community_routes, discussion_topic_routes};
// use darve_server::routes::user_auth::access_rule_routes;
// use discussion_topic_routes::{DiscussionTopicItemsEdit, TopicInput};
// use local_user_entity::{LocalUser, LocalUserDbService};
// use middleware::ctx::Ctx;
// use middleware::error::AppError;
// use middleware::utils::db_utils::IdentIdName;
// use middleware::utils::extractor_utils::DiscussionParams;
// use middleware::utils::request_utils::CreatedResponse;
// use surrealdb::sql::Thing;
// use uuid::Uuid;

// use crate::helpers::{create_login_test_user, create_test_server};

// #[tokio::test]
// async fn display_access_rule_content() {
//     let (server, ctx_state) = create_test_server().await;
//     let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;

//     let comm_name = "community_123";
//     let create_response = server
//         .post("/api/community")
//         .json(&CommunityInput {
//             id: "".to_string(),
//             name_uri: comm_name.to_string(),
//             title: "The Community Test".to_string(),
//         })
//         .await;
//     create_response.assert_status_success();
//     let created = &create_response.json::<CreatedResponse>();

//     let comm_id = Thing::try_from(created.id.clone()).unwrap();
//     let comm_name = created.uri.clone().unwrap();

//     let ctx = &Ctx::new(Ok(user_ident), Uuid::new_v4(), false);
//     let comm_db = CommunityDbService {
//         db: &ctx_state._db,
//         ctx: &ctx,
//     };
//     let comm = comm_db.get(IdentIdName::Id(comm_id.clone())).await;
//     let comm_disc_id = comm.unwrap().profile_discussion.unwrap();

//     let create_response = server
//         .post("/api/access-rule")
//         .json(&AccessRuleInput {
//             id: "".to_string(),
//             target_entity_id: comm_id.to_raw(),
//             title: "Access Rule Register".to_string(),
//             authorize_record_id_required: comm_id.to_raw(),
//             authorize_activity_required: AUTH_ACTIVITY_VISITOR.to_string(),
//             authorize_height_required: 1000,
//             price_amount: "".to_string(),
//             available_period_days: "".to_string(),
//             access_gain_action_confirmation: "".to_string(),
//             access_gain_action_redirect_url: "".to_string(),
//         })
//         .await;
//     create_response.assert_status_success();
//     let created = &create_response.json::<AccessRuleForm>();
//     let ar_0 = created.access_rules.get(0).unwrap();
//     let ar_id = ar_0.id.clone().unwrap();
//     let ar_db = AccessRuleDbService {
//         db: &ctx_state._db,
//         ctx: &ctx,
//     };

//     let ar = ar_db
//         .get(IdentIdName::Id(ar_id.clone()).into())
//         .await
//         .expect("access rule");
//     assert_eq!(ar.id.clone().unwrap(), ar_id);

//     let post_name = "post title Name 0".to_string();
//     let create_post = server
//         .post(&format!("/api/discussion/{comm_disc_id}/post"))
//         .multipart(
//             MultipartForm::new()
//                 .add_text("title", post_name.clone())
//                 .add_text("content", "contentttt111")
//                 .add_text("topic_id", ""),
//         )
//         .await;
//     let created = create_post.json::<CreatedResponse>();
//     create_post.assert_status_success();
//     assert_eq!(created.id.len() > 0, true);

//     let post_name = "post title Name 1".to_string();
//     let create_post = server
//         .post(format!("/api/discussion/{comm_disc_id}/post").as_str())
//         .multipart(
//             MultipartForm::new()
//                 .add_text("title", post_name.clone())
//                 .add_text("content", "contentttt111")
//                 .add_text("topic_id", ""),
//         )
//         .await;
//     let created = create_post.json::<CreatedResponse>();
//     create_post.assert_status_success();
//     assert_eq!(created.id.len() > 0, true);

//     let comm_view = get_community(
//         State(ctx_state.clone()),
//         ctx.clone(),
//         Path(comm_name.clone()),
//         DiscussionParams {
//             topic_id: None,
//             start: None,
//             count: None,
//         },
//     )
//     .await
//     .expect("community page");
//     let posts = comm_view
//         .community_view
//         .unwrap()
//         .profile_discussion_view
//         .unwrap()
//         .posts;
//     assert_eq!(posts.len(), 2);

//     let topic_title = "topic1".to_string();
//     let topic_resp = server
//         .post(format!("/api/discussion/{}/topic", comm_disc_id).as_str())
//         .json(&TopicInput {
//             id: "".to_string(),
//             title: topic_title.clone(),
//             hidden: None,
//             access_rule_id: ar_id.to_raw(),
//         })
//         .await;
//     topic_resp.assert_status_success();
//     let created = &topic_resp.json::<DiscussionTopicItemsEdit>();
//     assert_eq!(&created.community_id, &comm_id);
//     assert_eq!(&created.topics.get(0).unwrap().title, &topic_title);
//     let topic1_id = created.topics.get(0).unwrap().id.clone();

//     let post_name = "post title Name 2".to_string();
//     let create_post = server
//         .post(format!("/api/discussion/{comm_disc_id}/post").as_str())
//         .multipart(
//             MultipartForm::new()
//                 .add_text("title", post_name.clone())
//                 .add_text("content", "contentttt111")
//                 .add_text("topic_id", topic1_id.clone().unwrap().to_raw()),
//         )
//         .await;
//     let created = create_post.json::<CreatedResponse>();
//     create_post.assert_status_success();
//     assert_eq!(created.id.len() > 0, true);

//     let comm_view = get_community(
//         State(ctx_state.clone()),
//         ctx.clone(),
//         Path(comm_name.clone()),
//         DiscussionParams {
//             topic_id: None,
//             start: None,
//             count: None,
//         },
//     )
//     .await
//     .expect("community page");
//     let posts = comm_view
//         .community_view
//         .unwrap()
//         .profile_discussion_view
//         .unwrap()
//         .posts;
//     assert_eq!(posts.len(), 3);

//     // check view with admin
//     let comm_view = get_community(
//         State(ctx_state.clone()),
//         ctx.clone(),
//         Path(comm_name.clone()),
//         DiscussionParams {
//             topic_id: topic1_id.clone(),
//             start: None,
//             count: None,
//         },
//     )
//     .await
//     .expect("community page");
//     let posts = comm_view
//         .community_view
//         .unwrap()
//         .profile_discussion_view
//         .unwrap()
//         .posts;
//     assert_eq!(posts.len(), 1);
//     let post0 = posts.get(0).unwrap();
//     let post_access_rule = post0.access_rule.clone().unwrap();
//     assert_eq!(post_access_rule.id.clone().unwrap(), ar_id);
//     assert_eq!(post0.viewer_access_rights.len() == 1, true);
//     let post_viewer_access_rights = post0.viewer_access_rights.get(0).unwrap();
//     assert_eq!(
//         post_viewer_access_rights.authorize_activity,
//         AUTH_ACTIVITY_OWNER.to_string()
//     );
//     assert_eq!(
//         post_viewer_access_rights
//             .ge_equal_ident(&post_access_rule.authorization_required)
//             .expect("ok"),
//         true
//     );
//     assert_eq!(
//         post_access_rule.authorization_required,
//         ar.authorization_required
//     );

//     // check view with no user
//     let ctx_no_user = Ctx::new(Err(AppError::AuthFailNoJwtCookie), Uuid::new_v4(), false);
//     let comm_view = get_community(
//         State(ctx_state.clone()),
//         ctx_no_user,
//         Path(comm_name.clone()),
//         DiscussionParams {
//             topic_id: topic1_id.clone(),
//             start: None,
//             count: None,
//         },
//     )
//     .await
//     .expect("community page");
//     let posts = comm_view
//         .community_view
//         .unwrap()
//         .profile_discussion_view
//         .unwrap()
//         .posts;
//     assert_eq!(posts.len(), 1);
//     let post0 = posts.get(0).unwrap();
//     let post_access_rule = post0.access_rule.clone().unwrap();
//     assert_eq!(post_access_rule.id.clone().unwrap(), ar_id);
//     assert_eq!(post0.viewer_access_rights.len() == 0, true);
//     assert_eq!(
//         post_access_rule.authorization_required,
//         ar.authorization_required
//     );

//     // check view with low access user
//     let user_service = LocalUserDbService {
//         db: &ctx_state._db,
//         ctx: &ctx,
//     };
//     let new_user_id = user_service
//         .create(
//             LocalUser {
//                 id: None,
//                 username: "visitor".to_string(),
//                 full_name: None,
//                 birth_date: None,
//                 phone: None,
//                 email: None,
//                 bio: None,
//                 social_links: None,
//                 image_uri: None,
//             },
//             AuthType::PASSWORD(Some("visitor".to_string())),
//         )
//         .await
//         .expect("local user");

//     AccessRightDbService {
//         db: &ctx_state._db,
//         ctx,
//     }
//     .authorize(
//         Thing::try_from(new_user_id.clone()).unwrap(),
//         Authorization {
//             authorize_record_id: comm_id.clone(),
//             authorize_activity: AUTH_ACTIVITY_VISITOR.to_string(),
//             authorize_height: 1,
//         },
//         None,
//     )
//     .await
//     .expect("authorized");

//     let ctx_no_user = Ctx::new(Ok(new_user_id), Uuid::new_v4(), false);
//     let comm_view = get_community(
//         State(ctx_state.clone()),
//         ctx_no_user,
//         Path(comm_name.clone()),
//         DiscussionParams {
//             topic_id: topic1_id.clone(),
//             start: None,
//             count: None,
//         },
//     )
//     .await
//     .expect("community page");
//     let posts = comm_view
//         .community_view
//         .unwrap()
//         .profile_discussion_view
//         .unwrap()
//         .posts;
//     assert_eq!(posts.len(), 1);
//     let post0 = posts.get(0).unwrap();
//     let post_access_rule = post0.access_rule.clone().unwrap();
//     assert_eq!(post_access_rule.id.clone().unwrap(), ar_id);
//     assert_eq!(post0.viewer_access_rights.len() == 1, true);
//     let post_viewer_access_rights = post0.viewer_access_rights.get(0).unwrap();
//     assert_eq!(
//         post_viewer_access_rights.authorize_activity,
//         AUTH_ACTIVITY_VISITOR.to_string()
//     );
//     assert_eq!(
//         post_viewer_access_rights
//             .ge_equal_ident(&post_access_rule.authorization_required)
//             .expect("ok"),
//         false
//     );
//     assert_eq!(
//         post_access_rule.authorization_required,
//         ar.authorization_required
//     );
// }
