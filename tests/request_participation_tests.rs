mod helpers;
use axum::http::StatusCode;
use axum_test::multipart::MultipartForm;
use chrono::DateTime;
use darve_server::entities::community::community_entity;
use darve_server::entities::community::post_entity::Post;
use darve_server::entities::task::task_request_entity::TaskRequest;
use darve_server::entities::user_notification::{UserNotification, UserNotificationEvent};
use darve_server::entities::wallet::wallet_entity;
use darve_server::middleware;
use darve_server::middleware::utils::request_utils::CreatedResponse;
use darve_server::routes::community::community_routes;
use darve_server::routes::tasks::{TaskRequestOfferInput, TaskRequestView};
use darve_server::routes::user_auth::login_routes;
use darve_server::routes::wallet::wallet_routes;
use darve_server::services::task_service::TaskRequestInput;
use helpers::post_helpers::create_fake_post;
use serde_json::json;
use std::i64;
use surrealdb::sql::Thing;

use crate::helpers::{create_fake_login_test_user, create_login_test_user};
use community_entity::{Community, CommunityDbService};
use community_routes::CommunityInput;
use login_routes::LoginInput;
use middleware::ctx::Ctx;
use middleware::utils::string_utils::get_string_thing;
use wallet_entity::WalletDbService;
use wallet_routes::CurrencyTransactionHistoryView;

test_with_server!(
    create_task_request_participation,
    |server, ctx_state, config| {
        // let (server, user3, user3_pwd, _) = create_fake_login_test_user(&server).await;
        // let user_ident3 = user3.id.as_ref().unwrap().to_raw();
        // let username3 = user3.username.to_string();

        let username2 = "usnnnn2".to_string();
        let username4 = "usnnnn4".to_string();

        // let (server, user1, _, _) = create_fake_login_test_user(&server).await;

        let (server, user0, user0_pwd, _user0_token) = create_fake_login_test_user(&server).await;
        let user_ident0 = user0.id.as_ref().unwrap().to_raw();
        let username0 = user0.username.to_string();

        ////////// user 0 creates post (user 2 creates task and user3 participates on this post for user 0 who delivers it, user4 tries to participates without enough funds)

        // create community
        let create_response = server
            .post("/api/community")
            .json(&CommunityInput {
                id: "".to_string(),
                name_uri: "comm-naMMe1".to_lowercase(),
                title: "The Community Test".to_string(),
            })
            .add_header("Accept", "application/json")
            .await;
        let created = &create_response.json::<CreatedResponse>();

        let comm_id = Thing::try_from(created.id.clone()).unwrap();
        let comm_name = created.uri.clone().unwrap();
        create_response.assert_status_success();

        let ctx = Ctx::new(Ok(user_ident0.clone()), false);
        let community_db_service = CommunityDbService {
            db: &ctx_state.db.client,
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
            .post(format!("/api/discussions/{community_discussion_id}/posts").as_str())
            .multipart(
                MultipartForm::new()
                    .add_text("title", post_name.clone())
                    .add_text("content", "contentttt")
                    .add_text("topic_id", ""),
            )
            .add_header("Accept", "application/json")
            .await;
        let created_post = create_post.json::<Post>();
        create_post.assert_status_success();
        let post_id = created_post.id.as_ref().unwrap().to_raw();

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
            .post(&format!("/api/posts/{post_id}/tasks"))
            .json(&TaskRequestInput {
                offer_amount: Some(user2_offer_amt),
                participant: Some(user_ident0.clone()),
                content: offer_content.clone(),
                acceptance_period: None,
                delivery_period: None,
            })
            .add_header("Accept", "application/json")
            .await;

        task_request.assert_status_success();
        let created_task = task_request.json::<TaskRequest>();

        let post_tasks_req = server
            .get(&format!("/api/posts/{post_id}/tasks"))
            .add_header("Accept", "application/json")
            .await;

        post_tasks_req.assert_status_success();
        let post_tasks = post_tasks_req.json::<Vec<TaskRequestView>>();

        let task = post_tasks.get(0).unwrap();
        let offer0 = task.donors.get(0).unwrap();

        assert_eq!(created_task.id, task.id);

        assert_eq!(offer0.amount.clone(), user2_offer_amt);
        assert_eq!(task.created_by.username, username2);
        // assert_eq!(task.to_user.clone().unwrap().username, username0);
        assert_eq!(task.donors.len(), 1);
        assert_eq!(offer0.user.clone().unwrap().username, username2);

        // all tasks given by user
        let given_user_tasks_req = server
            .get("/api/tasks/given")
            .add_header("Accept", "application/json")
            .await;

        given_user_tasks_req.assert_status_success();
        let given_post_tasks = given_user_tasks_req.json::<Vec<TaskRequestView>>();

        assert_eq!(given_post_tasks.len(), 1);

        ////////// login user 3 and participate

        let (server, user3, user3_pwd, _) = create_fake_login_test_user(&server).await;
        let user3_thing = user3.id.unwrap();
        let username3 = user3.username.to_string();

        // endow user 3
        let user3_endow_amt: i64 = 100;
        let user3_offer_amt: i64 = 3;
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
            .post(format!("/api/tasks/{}/donor", task.id.clone().unwrap()).as_str())
            .json(&TaskRequestOfferInput {
                amount: user3_offer_amt as u64,
            })
            .add_header("Accept", "application/json")
            .await;
        participate_response.assert_status_success();

        let wallet_service = WalletDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        };
        let balance = wallet_service.get_user_balance(&user3_thing).await.unwrap();
        assert_eq!(balance.balance_usd, user3_endow_amt - user3_offer_amt);

        let post_tasks_req = server
            .get(format!("/api/posts/{}/tasks", post_id).as_str())
            .add_header("Accept", "application/json")
            .await;

        post_tasks_req.assert_status_success();
        let post_tasks = post_tasks_req.json::<Vec<TaskRequestView>>();

        let task = post_tasks.get(0).unwrap();
        assert_eq!(task.donors.len(), 2);
        let balance = wallet_service.get_balance(&task.wallet_id).await.unwrap();
        assert_eq!(balance.balance_usd, user2_offer_amt + user3_offer_amt);

        // change amount to 33 by sending another participation req
        let user3_offer_amt: i64 = 33;
        let participate_response = server
            .post(format!("/api/tasks/{}/donor", task.id.clone().unwrap()).as_str())
            .json(&TaskRequestOfferInput {
                amount: user3_offer_amt as u64,
            })
            .add_header("Accept", "application/json")
            .await;

        participate_response.assert_status_success();

        let wallet_service = WalletDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        };
        let balance = wallet_service.get_user_balance(&user3_thing).await.unwrap();
        assert_eq!(balance.balance_usd, user3_endow_amt - user3_offer_amt);

        let post_tasks_req = server
            .get(format!("/api/posts/{}/tasks", post_id).as_str())
            .add_header("Accept", "application/json")
            .await;

        post_tasks_req.assert_status_success();
        let post_tasks = post_tasks_req.json::<Vec<TaskRequestView>>();

        let task = post_tasks.get(0).unwrap();
        assert_eq!(task.donors.len(), 2);

        let task = post_tasks.get(0).unwrap();
        assert_eq!(task.donors.len(), 2);
        let balance = wallet_service.get_balance(&task.wallet_id).await.unwrap();
        assert_eq!(balance.balance_usd, user2_offer_amt + user3_offer_amt);

        // user4 tries to participate without balance and gets error

        let (server, user_ident4) = create_login_test_user(&server, username4.clone()).await;
        let user4_thing = get_string_thing(user_ident4).unwrap();
        let balance = wallet_service.get_user_balance(&user4_thing).await.unwrap();
        assert_eq!(balance.balance_usd, 0);

        let participate_response = server
            .post(format!("/api/tasks/{}/donor", task.id.clone().unwrap()).as_str())
            .json(&TaskRequestOfferInput { amount: 33 })
            .add_header("Accept", "application/json")
            .await;

        participate_response.assert_status_failure();
        participate_response.assert_status(StatusCode::PAYMENT_REQUIRED);

        ////////// login user 0 and check tasks

        server.get("/logout").await;
        let login_response = server
            .post("/api/login")
            .json(&LoginInput {
                username: username0.clone(),
                password: user0_pwd.clone(),
                next: None,
            })
            .add_header("Accept", "application/json")
            .await;
        login_response.assert_status_success();

        // check received tasks
        let received_post_tasks_req = server
            .get("/api/tasks/received")
            .add_header("Accept", "application/json")
            .await;

        received_post_tasks_req.assert_status_success();
        let received_post_tasks = received_post_tasks_req.json::<Vec<TaskRequestView>>();

        assert_eq!(received_post_tasks.len(), 1);
        let received_task = received_post_tasks.get(0).unwrap();
        // assert_eq!(received_task.status, TaskStatus::Requested.to_string());
        // assert_eq!(received_task.deliverables.is_none(), true);

        // accept received task
        let accept_response = server
            .post(format!("/api/tasks/{}/accept", received_task.id.clone().unwrap()).as_str())
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();

        // check task is accepted
        let received_post_tasks_req = server
            .get("/api/tasks/received")
            .add_header("Accept", "application/json")
            .await;

        received_post_tasks_req.assert_status_success();
        let received_post_tasks = received_post_tasks_req.json::<Vec<TaskRequestView>>();

        assert_eq!(received_post_tasks.len(), 1);
        // let received_task = received_post_tasks.get(0).unwrap();
        // assert_eq!(received_task.status, TaskStatus::Accepted.to_string());

        //////// deliver task

        // create post on own profile for task delivery
        let post_name = "delivery post".to_string();
        let create_post = server
            .post("/api/users/current/posts")
            .multipart(
                MultipartForm::new()
                    .add_text("title", post_name.clone())
                    .add_text("content", "delivery contentttt")
                    .add_text("topic_id", ""),
            )
            .add_header("Accept", "application/json")
            .await;
        let created_post = create_post.json::<Post>();
        create_post.assert_status_success();
        let delivery_post_id = created_post.id.as_ref().unwrap().to_raw();
        println!("DEL POST={}", delivery_post_id.clone());

        // deliver task
        let delivery_req = server
            .post(format!("/api/tasks/{}/deliver", received_task.id.clone().unwrap()).as_str())
            .json(&json!({"post_id": delivery_post_id}))
            .await;
        delivery_req.assert_status_success();

        let received_post_tasks_req = server
            .get("/api/tasks/received")
            .add_header("Accept", "application/json")
            .await;

        received_post_tasks_req.assert_status_success();
        let received_post_tasks = received_post_tasks_req.json::<Vec<TaskRequestView>>();
        let _task = received_post_tasks.get(0).unwrap();
        // assert_eq!(task.deliverables.clone().unwrap().is_empty(), false);

        // TODO -check notifications for other users-
        // login user3 to check notifications
        server.get("/logout").await;
        let login_response = server
            .post("/api/login")
            .json(&LoginInput {
                username: username3.clone(),
                password: user3_pwd.clone(),
                next: None,
            })
            .add_header("Accept", "application/json")
            .await;
        login_response.assert_status_success();

        // check user notifications
        let notif_history_req = server
            .get("/api/notifications")
            .add_header("Accept", "application/json")
            .await;

        notif_history_req.assert_status_success();
        let received_notifications = notif_history_req.json::<Vec<UserNotification>>();

        assert_eq!(received_notifications.len(), 4);

        let balance_updates: Vec<_> = received_notifications
            .iter()
            .filter(|v| v.event == UserNotificationEvent::UserBalanceUpdate)
            .collect();
        assert_eq!(balance_updates.len(), 3);
        let task_delivered_evt: Vec<_> = received_notifications
            .iter()
            .filter(|v| v.event == UserNotificationEvent::UserTaskRequestDelivered)
            .collect();
        assert_eq!(task_delivered_evt.len(), 1);

        // check transaction history /api/user/wallet/history
        let transaction_history_response = server
            .get("/api/wallet/history?start=0&count=20")
            .add_header("Accept", "application/json")
            .await;
        transaction_history_response.assert_status_success();

        let created = &transaction_history_response.json::<CurrencyTransactionHistoryView>();
        assert_eq!(created.transactions.len(), 4);

        created
            .transactions
            .iter()
            .fold(i64::MAX, |prev_val, tx_v| {
                let date_time = DateTime::parse_from_rfc3339(tx_v.r_created.as_str());
                let ts = date_time.unwrap().timestamp();
                println!(
                    "for {} with {} in {:?} out {:?} after tx balance={}",
                    tx_v.wallet.id,
                    tx_v.with_wallet.id,
                    tx_v.amount_in,
                    tx_v.amount_out,
                    tx_v.balance
                );
                assert_eq!(ts <= prev_val, true);
                ts
            });

        // check transaction history /api/user/wallet/history
        let transaction_history_response = server
            .get("/api/wallet/history?start=2&count=20")
            .add_header("Accept", "application/json")
            .await;
        transaction_history_response.assert_status_success();

        let created = &transaction_history_response.json::<CurrencyTransactionHistoryView>();
        assert_eq!(created.transactions.len(), 2);

        created
            .transactions
            .iter()
            .fold(i64::MAX, |prev_val, tx_v| {
                let date_time = DateTime::parse_from_rfc3339(tx_v.r_created.as_str());
                let ts = date_time.unwrap().timestamp();
                println!(
                    "for {} with {} in {:?} out {:?} after tx balance={}",
                    tx_v.wallet.id,
                    tx_v.with_wallet.id,
                    tx_v.amount_in,
                    tx_v.amount_out,
                    tx_v.balance
                );
                assert_eq!(ts <= prev_val, true);
                ts
            });
    }
);

test_with_server!(get_notifications, |server, ctx_state, config| {
    let (_, _user, _password, token) = create_fake_login_test_user(&server).await;
    let (_, user1, _password, token1) = create_fake_login_test_user(&server).await;
    let discussion_id = Thing::from((
        "discussion".to_string(),
        user1.id.as_ref().unwrap().id.to_raw(),
    ));

    let create_response = server
        .post(&format!(
            "/api/followers/{}",
            user1.id.as_ref().unwrap().to_raw()
        ))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    create_response.assert_status_success();

    let _ = create_fake_post(&server, &discussion_id, None, None).await;
    let _ = create_fake_post(&server, &discussion_id, None, None).await;
    let _ = create_fake_post(&server, &discussion_id, None, None).await;
    let _ = create_fake_post(&server, &discussion_id, None, None).await;

    // TODO -need to follow to get post notifications-

    let req = server
        .get("/api/notifications")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;

    req.assert_status_success();
    let notifications = req.json::<Vec<UserNotification>>();
    assert_eq!(notifications.len(), 4);
    let req = server
        .get("/api/notifications?count=1")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;

    req.assert_status_success();
    let notifications = req.json::<Vec<UserNotification>>();
    assert_eq!(notifications.len(), 1);
    let req = server
        .get("/api/notifications?is_read=true")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;

    req.assert_status_success();
    let notifications = req.json::<Vec<UserNotification>>();
    assert_eq!(notifications.len(), 0);

    let req = server
        .get("/api/notifications")
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;

    req.assert_status_success();
    let notifications = req.json::<Vec<UserNotification>>();
    assert_eq!(notifications.len(), 1);
});

test_with_server!(set_read_notification, |server, ctx_state, config| {
    let (_, _user, _password, token) = create_fake_login_test_user(&server).await;
    let (_, user1, _password, _token1) = create_fake_login_test_user(&server).await;
    let discussion_id = Thing::from((
        "discussion".to_string(),
        user1.id.as_ref().unwrap().id.to_raw(),
    ));
    let create_response = server
        .post(&format!(
            "/api/followers/{}",
            user1.id.as_ref().unwrap().to_raw()
        ))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    create_response.assert_status_success();
    let _ = create_fake_post(&server, &discussion_id, None, None).await;
    let req = server
        .get("/api/notifications")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;

    req.assert_status_success();
    let notifications = req.json::<Vec<UserNotification>>();
    assert_eq!(notifications.len(), 1);

    let id = &notifications.first().as_ref().unwrap().id;
    let req = server
        .post(&format!("/api/notifications/{id}/read"))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    req.assert_status_success();

    let req = server
        .get("/api/notifications")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;

    req.assert_status_success();
    let notifications = req.json::<Vec<UserNotification>>();
    assert_eq!(notifications.len(), 1);
    let first = notifications.first().unwrap();
    assert_eq!(first.is_read, true);
});

test_with_server!(set_read_all_notifications, |server, ctx_state, config| {
    let (_, _user, _password, token) = create_fake_login_test_user(&server).await;
    let (_, user1, _password, _token1) = create_fake_login_test_user(&server).await;
    let discussion_id = Thing::from((
        "discussion".to_string(),
        user1.id.as_ref().unwrap().id.to_raw(),
    ));

    let create_response = server
        .post(&format!(
            "/api/followers/{}",
            user1.id.as_ref().unwrap().to_raw()
        ))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    create_response.assert_status_success();
    let _ = create_fake_post(&server, &discussion_id, None, None).await;
    let _ = create_fake_post(&server, &discussion_id, None, None).await;
    let _ = create_fake_post(&server, &discussion_id, None, None).await;
    let _ = create_fake_post(&server, &discussion_id, None, None).await;
    let _ = create_fake_post(&server, &discussion_id, None, None).await;
    let req = server
        .get("/api/notifications")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;

    req.assert_status_success();
    let notifications = req.json::<Vec<UserNotification>>();
    assert_eq!(notifications.len(), 5);

    let req = server
        .post(&format!("/api/notifications/read"))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    req.assert_status_success();

    let req = server
        .get("/api/notifications?is_read=true")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;

    req.assert_status_success();
    let notifications = req.json::<Vec<UserNotification>>();
    assert_eq!(notifications.len(), 5);
});

test_with_server!(get_count_of_notifications, |server, ctx_state, config| {
    let (_, _user, _password, token) = create_fake_login_test_user(&server).await;
    let (_, user1, _password, _token1) = create_fake_login_test_user(&server).await;
    let discussion_id = Thing::from((
        "discussion".to_string(),
        user1.id.as_ref().unwrap().id.to_raw(),
    ));
    let create_response = server
        .post(&format!(
            "/api/followers/{}",
            user1.id.as_ref().unwrap().to_raw()
        ))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    create_response.assert_status_success();
    let _ = create_fake_post(&server, &discussion_id, None, None).await;
    let _ = create_fake_post(&server, &discussion_id, None, None).await;
    let _ = create_fake_post(&server, &discussion_id, None, None).await;
    let _ = create_fake_post(&server, &discussion_id, None, None).await;
    let _ = create_fake_post(&server, &discussion_id, None, None).await;
    let req = server
        .get("/api/notifications/count")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;

    req.assert_status_success();
    let count = req.json::<u64>();
    assert_eq!(count, 5);

    let req = server
        .get("/api/notifications/count?is_read=true")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    req.assert_status_success();

    req.assert_status_success();
    let count = req.json::<u64>();
    assert_eq!(count, 0);
});
