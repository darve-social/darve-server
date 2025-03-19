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
    use sb_task::entity::task_request_offer_entity::TaskOfferParticipantDbService;
    use sb_task::routes::task_request_routes::{AcceptTaskRequestInput, TaskRequestInput, TaskRequestOfferInput, TaskRequestView};
    use sb_user_auth::routes::login_routes::LoginInput;
    use sb_wallet::entity::funding_transaction_entity::FundingTransactionDbService;
    use sb_wallet::entity::wallet_entitiy::{CurrencySymbol, WalletDbService};

    #[tokio::test]
    async fn create_request_offer() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let username0 = "usnnnn0".to_string();
        let username1 = "usnnnn1".to_string();
        let username2 = "usnnnn2".to_string();
        let username3 = "usnnnn3".to_string();
        let (server, user_ident0) = create_login_test_user(&server, username0.clone()).await;
        let (server, user_ident3) = create_login_test_user(&server, username3.clone()).await;
        let (server, user_ident1) = create_login_test_user(&server, username1.clone()).await;

        let comm_name = "comm-naMMe1".to_lowercase();


        ////////// user 1 creates post (user 2 creates task on this post for user 0 who delivers it)


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


        ////////// user 2 creates offer for user 0


        let (server, user_ident2) = create_login_test_user(&server, username2.clone()).await;
        let user2_thing = get_string_thing(user_ident2).unwrap();

        let fund_service = FundingTransactionDbService { db: &ctx_state._db, ctx: &ctx };
        fund_service.user_endowment_tx(&user2_thing, "ext_acc123".to_string(), "ext_tx_id_123".to_string(), 100, CurrencySymbol::USD).await.expect("created");
        let offer_amount = Some(2);
        let offer_content = "contdad".to_string();
        let task_request = server
            .post("/api/task_request")
            .json(&TaskRequestInput { post_id: Some(created_post.id.clone()), offer_amount: offer_amount.clone(), to_user: user_ident0.clone(), content: offer_content.clone() })
            .add_header("Accept", "application/json")
            .await;

        &task_request.assert_status_success();
        let created_task = task_request.json::<CreatedResponse>();

        let post_tasks_req = server
            .get(format!("/api/task_request/list/post/{}", created_post.id.clone()).as_str())
            .add_header("Accept", "application/json")
            .await;

        &post_tasks_req.assert_status_success();
        let post_tasks = post_tasks_req.json::<Vec<TaskRequestView>>();

        let task = post_tasks.get(0).unwrap();
        let offer0 = task.participants.get(0).unwrap();

        assert_eq!(created_task.id, task.id.clone().unwrap().to_raw());

        assert_eq!(offer0.participants.get(0).unwrap().amount, offer_amount.unwrap());
        assert_eq!(post_tasks.get(0).unwrap().from_user.username, username2);
        assert_eq!(post_tasks.get(0).unwrap().to_user.username, username0);
        assert_eq!(offer0.participants.len(), 1);
        assert_eq!(offer0.participants.get(0).unwrap().user.username, username2);

        // all tasks given by user
        let given_user_tasks_req = server
            .get("/api/task_request/given")
            .add_header("Accept", "application/json")
            .await;

        &given_user_tasks_req.assert_status_success();
        let given_post_tasks = given_user_tasks_req.json::<Vec<TaskRequestView>>();

        assert_eq!(given_post_tasks.len(), 1);


        ////////// login user 3 and participate


        server.get("/logout").await;
        let login_response = server
            .post("/api/login")
            .json(&LoginInput {
                username: username3.clone(),
                password: "some3242paSs#$".to_string(),
                next: None,
            })
            .add_header("Accept", "application/json")
            .await;
        login_response.assert_status_success();
        
        let user3_thing = get_string_thing(user_ident3).unwrap();
        let fund_service = FundingTransactionDbService { db: &ctx_state._db, ctx: &ctx };
        fund_service.user_endowment_tx(&user3_thing.clone(), "ext_acc123".to_string(), "ext_tx_id_123".to_string(), 100, CurrencySymbol::USD).await.expect("created");

// TODO check if balance was locked
        let participate_response = server
            .post(format!("/api/task_offer/{}/participate", offer0.id.clone().unwrap()).as_str())
            .json(&TaskRequestOfferInput {
                amount: 3,
                currency: Some(CurrencySymbol::USD),
            })
            .add_header("Accept", "application/json")
            .await;

        participate_response.assert_status_success();
        let res = participate_response.json::<CreatedResponse>();


        let wallet_service = WalletDbService { db: &ctx_state._db, ctx: &ctx };
        let balance = wallet_service.get_user_balance(&user3_thing).await.unwrap();
        let balance_locked = wallet_service.get_user_balance_locked(&user3_thing).await.unwrap();
        assert_eq!(balance.balance_usd, 97);
        assert_eq!(balance_locked.balance_usd, 3);
        
        let post_tasks_req = server
            .get(format!("/api/task_request/list/post/{}", created_post.id.clone()).as_str())
            .add_header("Accept", "application/json")
            .await;

        &post_tasks_req.assert_status_success();
        let post_tasks = post_tasks_req.json::<Vec<TaskRequestView>>();

        let task = post_tasks.get(0).unwrap();
        let offer0 = task.offers.get(0).unwrap();
        assert_eq!(offer0.participants.len(), 2);
        let participant = task.offers.get(0).unwrap().participants.iter().find(|p| p.user.username == username3).unwrap();
        assert_eq!(participant.amount, 3);

        // change amount to 33 by sending another participation req
        let participate_response = server
            .post(format!("/api/task_offer/{}/participate", offer0.id.clone().unwrap()).as_str())
            .json(&TaskRequestOfferInput {
                amount: 33,
                currency: Some(CurrencySymbol::USD),
            })
            .add_header("Accept", "application/json")
            .await;

        participate_response.assert_status_success();
        let res = participate_response.json::<CreatedResponse>();

        let wallet_service = WalletDbService { db: &ctx_state._db, ctx: &ctx };
        let balance = wallet_service.get_user_balance(&user3_thing).await.unwrap();
        let balance_locked = wallet_service.get_user_balance_locked(&user3_thing).await.unwrap();
        assert_eq!(balance.balance_usd, 67);
        assert_eq!(balance_locked.balance_usd, 33);

        let post_tasks_req = server
            .get(format!("/api/task_request/list/post/{}", created_post.id.clone()).as_str())
            .add_header("Accept", "application/json")
            .await;

        &post_tasks_req.assert_status_success();
        let post_tasks = post_tasks_req.json::<Vec<TaskRequestView>>();

        let task = post_tasks.get(0).unwrap();
        let offer0 = task.offers.get(0).unwrap();
        assert_eq!(offer0.participants.len(), 2);
        let participant = task.offers.get(0).unwrap().participants.iter().find(|p| p.user.username == username3).unwrap();
        assert_eq!(participant.amount, 33);


        ////////// login user 0 and check tasks


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


        //////// deliver task (called directly so file is not used)


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
                None,
            )
            .await.unwrap();
        assert_eq!(task.0.status, TaskStatus::Delivered.to_string());
        // dbg!(&task);

        let binding = task.0.deliverables.unwrap();
        let deliverable = binding.get(0).unwrap();
        assert_eq!(!deliverable.id.to_raw().is_empty(), true);

        let received_post_tasks_req = server
            .get("/api/task_request/received")
            .add_header("Accept", "application/json")
            .await;

        &received_post_tasks_req.assert_status_success();
        let received_post_tasks = received_post_tasks_req.json::<Vec<TaskRequestView>>();
        let task = received_post_tasks.get(0).unwrap();
        assert_eq!(task.deliverables.clone().unwrap().is_empty(), false);

    }
}
