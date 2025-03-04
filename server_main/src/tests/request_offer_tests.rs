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
    use sb_middleware::utils::string_utils::get_string_thing;
    use sb_task::entity::task_request_entitiy::{TaskRequest, TaskRequestDbService};
    use sb_task::entity::task_request_entitiy::TaskStatus;
    use sb_task::entity::task_request_offer_entity::TaskRequestOfferDbService;
    use sb_task::routes::task_request_routes::{AcceptTaskRequestInput, TaskRequestInput, TaskRequestView};
    use sb_user_auth::routes::login_routes::LoginInput;

    #[tokio::test]
    async fn create_request_offer() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let username0 = "usnnnn0".to_string();
        let username1 = "usnnnn1".to_string();
        let username2 = "usnnnn2".to_string();
        let (server, user_ident0) = create_login_test_user(&server, username0.clone()).await;
        let (server, user_ident1) = create_login_test_user(&server, username1.clone()).await;

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
        &create_response.assert_status_success();

        let ctx = Ctx::new(Ok(user_ident1.clone()), Uuid::new_v4(), false);
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
        let created_post = create_post.json::<CreatedResponse>();
        &create_post.assert_status_success();
        assert_eq!(created_post.id.len() > 0, true);

        let ctx_no_user = Ctx::new(Ok(user_ident1.clone()), Uuid::new_v4(), false);

        // user 2 creates offer for user 0
        let (server, user_ident2) = create_login_test_user(&server, username2.clone()).await;

        let offer_amount = Some(11);
        let offer_content = "contdad".to_string();
        let task_request = server
            .post("/api/task_request")
            .json(&TaskRequestInput { post_id: Some(created_post.id.clone()), offer_amount: offer_amount.clone(), to_user: user_ident0.clone(), content: offer_content.clone() })
            .add_header("Accept", "application/json")
            .await;

        let created_task = task_request.json::<CreatedResponse>();
        &task_request.assert_status_success();

        let post_tasks_req = server
            .get(format!("/api/task_request/list/post/{}", created_post.id.clone()).as_str())
            .add_header("Accept", "application/json")
            .await;

        let post_tasks = post_tasks_req.json::<Vec<TaskRequestView>>();
        &post_tasks_req.assert_status_success();

        let task = post_tasks.get(0).unwrap();

        assert_eq!(created_task.id, task.id.clone().unwrap().to_raw());
        assert_eq!(post_tasks.get(0).unwrap().offers.get(0).unwrap().participants.get(0).unwrap().amount, offer_amount.unwrap());
        assert_eq!(post_tasks.get(0).unwrap().from_user.username, username2);
        assert_eq!(post_tasks.get(0).unwrap().to_user.username, username0);
        assert_eq!(post_tasks.get(0).unwrap().offers.get(0).unwrap().participants.get(0).unwrap().user.username, username2);

        // all tasks given by user
        let given_user_tasks_req = server
            .get("/api/task_request/given")
            .add_header("Accept", "application/json")
            .await;

        &given_user_tasks_req.assert_status_success();
        let given_post_tasks = given_user_tasks_req.json::<Vec<TaskRequestView>>();

        assert_eq!(given_post_tasks.len(), 1);

        // login user 0 and check tasks
        server.get("/logout").await;
        let login_response = server
            .post("/api/login")
            .json(&LoginInput {
                username: username0.clone(),
                password: "some3242paSs#$".to_string(),
                next: None,
            })
            .add_header("Accept", "application/json")
            .await;
        login_response.assert_status_success();

        let received_post_tasks_req = server
            .get("/api/task_request/received")
            .add_header("Accept", "application/json")
            .await;

        &received_post_tasks_req.assert_status_success();
        let received_post_tasks = received_post_tasks_req.json::<Vec<TaskRequestView>>();

        assert_eq!(received_post_tasks.len(), 1);
        let received_task = received_post_tasks.get(0).unwrap();
        assert_eq!(received_task.status, TaskStatus::Requested.to_string());
        assert_eq!(received_task.deliverables.is_none(), true);

        let accept_response = server
            .post(format!("/api/task_request/{}/accept",received_task.id.clone().unwrap()).as_str())
            .json(&AcceptTaskRequestInput {
                accept: true,
            })
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();

        let received_post_tasks_req = server
            .get("/api/task_request/received")
            .add_header("Accept", "application/json")
            .await;

        &received_post_tasks_req.assert_status_success();
        let received_post_tasks = received_post_tasks_req.json::<Vec<TaskRequestView>>();

        assert_eq!(received_post_tasks.len(), 1);
        let received_task = received_post_tasks.get(0).unwrap();
        assert_eq!(received_task.status, TaskStatus::Accepted.to_string());

        let deliverables = vec!["/deliverable/file/uri".to_string()];
        let task = TaskRequestDbService {
            db: &ctx_state._db,
            ctx: &ctx,
        }
            .update_status_received_by_user(
                get_string_thing(user_ident0.clone()).expect("id"),
                received_task.id.clone().unwrap(),
                TaskStatus::Delivered,
                Some(deliverables.clone()),
            )
            .await.unwrap();
        assert_eq!(task.status, TaskStatus::Delivered.to_string());
        dbg!(&task);

        let binding = task.deliverables.unwrap();
        let deliverable = binding.get(0).unwrap();
        assert_eq!(!deliverable.id.to_raw().is_empty(), true);

        let received_post_tasks_req = server
            .get("/api/task_request/received")
            .add_header("Accept", "application/json")
            .await;

        &received_post_tasks_req.assert_status_success();
        let received_post_tasks = received_post_tasks_req.json::<Vec<TaskRequestView>>();
        dbg!(&received_post_tasks);
        // assert_eq!(received_post_tasks.get(0).unwrap().deliverables.unwrap().is_empty(), false);

    }
}
