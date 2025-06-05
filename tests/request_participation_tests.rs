mod helpers;

use axum::http::StatusCode;
use axum_test::multipart::MultipartForm;
use chrono::DateTime;
use darve_server::entities::task::task_request_entity;
use darve_server::entities::user_auth::user_notification_entity;
use darve_server::entities::wallet::wallet_entity;
use darve_server::middleware;
use darve_server::routes::community::community_routes;
use darve_server::routes::task::task_request_routes;
use darve_server::routes::user_auth::login_routes;
use darve_server::routes::wallet::wallet_routes;
use darve_server::{
    entities::community::community_entity, routes::user_auth::user_notification_routes,
};
use surrealdb::sql::Thing;
use uuid::Uuid;

use crate::helpers::{create_login_test_user, create_test_server};
use community_entity::{Community, CommunityDbService};
use community_routes::CommunityInput;
use darve_server::entities::wallet::gateway_transaction_entity::GatewayTransactionDbService;
use login_routes::LoginInput;
use middleware::ctx::Ctx;
use middleware::utils::db_utils::NO_SUCH_THING;
use middleware::utils::request_utils::CreatedResponse;
use middleware::utils::string_utils::get_string_thing;
use task_request_entity::TaskStatus;
use task_request_routes::{
    AcceptTaskRequestInput, TaskRequestInput, TaskRequestOfferInput, TaskRequestView,
};
use user_notification_entity::UserNotificationEvent;
use user_notification_routes::UserNotificationView;
use wallet_entity::{CurrencySymbol, WalletDbService};
use wallet_routes::CurrencyTransactionHistoryView;

#[tokio::test]
async fn create_task_request_participation() {
    let (server, ctx_state) = create_test_server().await;
    let username0 = "usnnnn0".to_string();
    let username1 = "usnnnn1".to_string();
    let username2 = "usnnnn2".to_string();
    let username3 = "usnnnn3".to_string();
    let username4 = "usnnnn4".to_string();
    let (server, user_ident0) = create_login_test_user(&server, username0.clone()).await;
    let (server, user_ident3) = create_login_test_user(&server, username3.clone()).await;
    let (server, user_ident1) = create_login_test_user(&server, username1.clone()).await;

    let comm_name = "comm-naMMe1".to_lowercase();

    ////////// user 1 creates post (user 2 creates task and user3 participates on this post for user 0 who delivers it, user4 tries to participates without enough funds)

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
    create_response.assert_status_success();

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
    let community_discussion_id = community.default_discussion.clone().unwrap();

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
    create_post.assert_status_success();
    assert_eq!(created_post.id.len() > 0, true);

    let _ = Ctx::new(Ok(user_ident1.clone()), Uuid::new_v4(), false);

    ////////// user 2 creates offer for user 0

    let (server, user_ident2) = create_login_test_user(&server, username2.clone()).await;
    let user2_thing = get_string_thing(user_ident2).unwrap();

    // endow user 2
    let user2_endow_amt = 100;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user2_thing.to_string(),
            user2_endow_amt
        ))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    endow_user_response.assert_status_success();

    let user2_offer_amt = 2;
    let offer_content = "contdad".to_string();
    let task_request = server
        .post("/api/task_request")
        .json(&TaskRequestInput {
            post_id: Some(created_post.id.clone()),
            offer_amount: Some(user2_offer_amt),
            to_user: user_ident0.clone(),
            content: offer_content.clone(),
        })
        .add_header("Accept", "application/json")
        .await;
    dbg!(&task_request);
    task_request.assert_status_success();
    let created_task = task_request.json::<CreatedResponse>();

    let post_tasks_req = server
        .get(format!("/api/task_request/list/post/{}", created_post.id.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;

    post_tasks_req.assert_status_success();
    let post_tasks = post_tasks_req.json::<Vec<TaskRequestView>>();

    let task = post_tasks.get(0).unwrap();
    let offer0 = task.participants.get(0).unwrap();

    assert_eq!(created_task.id, task.id.clone().unwrap().to_raw());

    assert_eq!(offer0.amount.clone(), user2_offer_amt);
    assert_eq!(task.from_user.username, username2);
    assert_eq!(task.to_user.clone().unwrap().username, username0);
    assert_eq!(task.participants.len(), 1);
    assert_eq!(offer0.user.clone().unwrap().username, username2);

    // all tasks given by user
    let given_user_tasks_req = server
        .get("/api/task_request/given")
        .add_header("Accept", "application/json")
        .await;

    given_user_tasks_req.assert_status_success();
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

    // endow user 3
    let user3_endow_amt = 100;
    let user3_offer_amt = 3;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user3_thing.to_string(),
            user3_endow_amt
        ))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    endow_user_response.assert_status_success();

    let participate_response = server
        .post(format!("/api/task_offer/{}/participate", task.id.clone().unwrap()).as_str())
        .json(&TaskRequestOfferInput {
            amount: user3_offer_amt,
            currency: Some(CurrencySymbol::USD),
        })
        .add_header("Accept", "application/json")
        .await;

    participate_response.assert_status_success();
    let _res = participate_response.json::<CreatedResponse>();

    let wallet_service = WalletDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };
    let balance = wallet_service.get_user_balance(&user3_thing).await.unwrap();
    let balance_locked = wallet_service
        .get_user_balance_locked(&user3_thing)
        .await
        .unwrap();
    assert_eq!(balance.balance_usd, user3_endow_amt - user3_offer_amt);
    assert_eq!(balance_locked.balance_usd, user3_offer_amt);

    let post_tasks_req = server
        .get(format!("/api/task_request/list/post/{}", created_post.id.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;

    post_tasks_req.assert_status_success();
    let post_tasks = post_tasks_req.json::<Vec<TaskRequestView>>();

    let task = post_tasks.get(0).unwrap();
    assert_eq!(task.participants.len(), 2);
    let participant = task
        .participants
        .iter()
        .find(|p| p.user.clone().unwrap().username == username3)
        .unwrap();
    assert_eq!(participant.amount, user3_offer_amt);

    // change amount to 33 by sending another participation req
    let user3_offer_amt = 33;
    let participate_response = server
        .post(format!("/api/task_offer/{}/participate", task.id.clone().unwrap()).as_str())
        .json(&TaskRequestOfferInput {
            amount: user3_offer_amt,
            currency: Some(CurrencySymbol::USD),
        })
        .add_header("Accept", "application/json")
        .await;

    participate_response.assert_status_success();
    let _res = participate_response.json::<CreatedResponse>();

    let wallet_service = WalletDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };
    let balance = wallet_service.get_user_balance(&user3_thing).await.unwrap();
    let balance_locked = wallet_service
        .get_user_balance_locked(&user3_thing)
        .await
        .unwrap();
    assert_eq!(balance.balance_usd, user3_endow_amt - user3_offer_amt);
    assert_eq!(balance_locked.balance_usd, user3_offer_amt);

    let post_tasks_req = server
        .get(format!("/api/task_request/list/post/{}", created_post.id.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;

    post_tasks_req.assert_status_success();
    let post_tasks = post_tasks_req.json::<Vec<TaskRequestView>>();

    let task = post_tasks.get(0).unwrap();
    assert_eq!(task.participants.len(), 2);
    let total_task_payment_amt = task.participants.iter().fold(0, |tot, a| tot + a.amount);
    assert_eq!(total_task_payment_amt, user3_offer_amt + user2_offer_amt);
    let participant = task
        .participants
        .iter()
        .find(|p| p.user.clone().unwrap().username == username3)
        .unwrap();
    assert_eq!(participant.amount, user3_offer_amt);


    // user4 tries to participate without balance and gets error
    
    let (server, user_ident4) = create_login_test_user(&server, username4.clone()).await;
    let user4_thing = get_string_thing(user_ident4).unwrap();
    let balance = wallet_service.get_user_balance(&user4_thing).await.unwrap();
    assert_eq!(balance.balance_usd, 0);

    let participate_response = server
        .post(format!("/api/task_offer/{}/participate", task.id.clone().unwrap()).as_str())
        .json(&TaskRequestOfferInput {
            amount: user3_offer_amt,
            currency: Some(CurrencySymbol::USD),
        })
        .add_header("Accept", "application/json")
        .await;

    participate_response.assert_status(StatusCode::PAYMENT_REQUIRED);

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

    // check received tasks
    let received_post_tasks_req = server
        .get("/api/task_request/received")
        .add_header("Accept", "application/json")
        .await;

    received_post_tasks_req.assert_status_success();
    let received_post_tasks = received_post_tasks_req.json::<Vec<TaskRequestView>>();

    assert_eq!(received_post_tasks.len(), 1);
    let received_task = received_post_tasks.get(0).unwrap();
    assert_eq!(received_task.status, TaskStatus::Requested.to_string());
    assert_eq!(received_task.deliverables.is_none(), true);

    // accept received task
    let accept_response = server
        .post(
            format!(
                "/api/task_request/{}/accept",
                received_task.id.clone().unwrap()
            )
            .as_str(),
        )
        .json(&AcceptTaskRequestInput { accept: true })
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_success();

    // check task is accepted
    let received_post_tasks_req = server
        .get("/api/task_request/received")
        .add_header("Accept", "application/json")
        .await;

    received_post_tasks_req.assert_status_success();
    let received_post_tasks = received_post_tasks_req.json::<Vec<TaskRequestView>>();

    assert_eq!(received_post_tasks.len(), 1);
    let received_task = received_post_tasks.get(0).unwrap();
    assert_eq!(received_task.status, TaskStatus::Accepted.to_string());

    //////// deliver task

    // create post on own profile for task delivery
    let post_name = "delivery post".to_string();
    let create_post = server
        .post("/api/user/post")
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "delivery contentttt")
                .add_text("topic_id", ""),
        )
        .add_header("Accept", "application/json")
        .await;
    let created_post = create_post.json::<CreatedResponse>();
    create_post.assert_status_success();
    let delivery_post_id = created_post.id.clone();
    println!("DEL POST={}", delivery_post_id.clone());
    assert_eq!(created_post.id.len() > 0, true);

    // deliver task
    let delivery_data = MultipartForm::new().add_text("post_id", delivery_post_id);
    let delivery_req = server
        .post(
            format!(
                "/api/task_request/{}/deliver",
                received_task.id.clone().unwrap()
            )
            .as_str(),
        )
        .multipart(delivery_data)
        .await;
    delivery_req.assert_status_success();

    // check user 3 balance and no locked
    let balance = wallet_service.get_user_balance(&user3_thing).await.unwrap();
    let balance_locked = wallet_service
        .get_user_balance_locked(&user3_thing)
        .await
        .unwrap();
    assert_eq!(balance.balance_usd, user3_endow_amt - user3_offer_amt);
    assert_eq!(balance_locked.balance_usd, 0);

    // check user 2 balance and no locked
    let balance = wallet_service.get_user_balance(&user2_thing).await.unwrap();
    let balance_locked = wallet_service
        .get_user_balance_locked(&user2_thing)
        .await
        .unwrap();
    assert_eq!(balance.balance_usd, user2_endow_amt - user2_offer_amt);
    assert_eq!(balance_locked.balance_usd, 0);

    // check user 0 has received rewards
    let user0_thing = get_string_thing(user_ident0).unwrap();
    let balance = wallet_service.get_user_balance(&user0_thing).await.unwrap();
    let balance_locked = wallet_service
        .get_user_balance_locked(&user0_thing)
        .await
        .unwrap();
    assert_eq!(balance.balance_usd, user3_offer_amt + user2_offer_amt);
    assert_eq!(balance_locked.balance_usd, 0);

    // check task deliverables exist
    let received_post_tasks_req = server
        .get("/api/task_request/received")
        .add_header("Accept", "application/json")
        .await;

    received_post_tasks_req.assert_status_success();
    let received_post_tasks = received_post_tasks_req.json::<Vec<TaskRequestView>>();
    let task = received_post_tasks.get(0).unwrap();
    assert_eq!(task.deliverables.clone().unwrap().is_empty(), false);

    // TODO check notifications for other users
    // login user3 to check notifications
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

    // check user notifications
    let notif_history_req = server
        .get("/api/notification/user/history")
        .add_header("Accept", "application/json")
        .await;

    notif_history_req.assert_status_success();
    let received_notifications = notif_history_req.json::<Vec<UserNotificationView>>();
    assert_eq!(received_notifications.len(), 5);

    let balance_updates: Vec<_> = received_notifications
        .iter()
        .filter(|v| v.event.to_string() == UserNotificationEvent::UserBalanceUpdate.to_string())
        .collect();
    assert_eq!(balance_updates.len(), 4);
    let balance_updates: Vec<_> = received_notifications
        .iter()
        .filter(|v| {
            v.event.to_string()
                == UserNotificationEvent::UserTaskRequestDelivered {
                    task_id: NO_SUCH_THING.clone(),
                    deliverable: NO_SUCH_THING.clone(),
                    delivered_by: NO_SUCH_THING.clone(),
                }
                .to_string()
        })
        .collect();
    assert_eq!(balance_updates.len(), 1);

    // check transaction history /api/user/wallet/history
    let transaction_history_response = server
        .get("/api/user/wallet/history?start=0&count=20")
        .add_header("Accept", "application/json")
        .await;
    transaction_history_response.assert_status_success();

    let created = &transaction_history_response.json::<CurrencyTransactionHistoryView>();
    assert_eq!(created.transactions.len(), 6);

    created.transactions.iter().fold(0i64, |prev_val, tx_v| {
        let date_time = DateTime::parse_from_rfc3339(tx_v.r_created.as_str());
        let ts = date_time.unwrap().timestamp();
        println!(
            "for {} with {} in {:?} out {:?} after tx balance={}",
            tx_v.wallet.id, tx_v.with_wallet.id, tx_v.amount_in, tx_v.amount_out, tx_v.balance
        );
        assert_eq!(ts >= prev_val, true);
        ts
    });

    // check transaction history /api/user/wallet/history
    let transaction_history_response = server
        .get("/api/user/wallet/history?start=2&count=20")
        .add_header("Accept", "application/json")
        .await;
    transaction_history_response.assert_status_success();

    let created = &transaction_history_response.json::<CurrencyTransactionHistoryView>();
    assert_eq!(created.transactions.len(), 4);

    created.transactions.iter().fold(0i64, |prev_val, tx_v| {
        let date_time = DateTime::parse_from_rfc3339(tx_v.r_created.as_str());
        let ts = date_time.unwrap().timestamp();
        println!(
            "for {} with {} in {:?} out {:?} after tx balance={}",
            tx_v.wallet.id, tx_v.with_wallet.id, tx_v.amount_in, tx_v.amount_out, tx_v.balance
        );
        assert_eq!(ts >= prev_val, true);
        ts
    });
    
}
