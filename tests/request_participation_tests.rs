mod helpers;
use axum::http::StatusCode;
use axum_test::multipart::MultipartForm;
use darve_server::entities::community::community_entity;
use darve_server::entities::community::discussion_entity::{Discussion, DiscussionDbService};
use darve_server::entities::community::post_entity::Post;
use darve_server::entities::task::task_request_entity::TaskRequest;
use darve_server::entities::user_notification::UserNotificationEvent;
use darve_server::entities::wallet::wallet_entity;
use darve_server::middleware;
use darve_server::models::view::balance_tx::CurrencyTransactionView;
use darve_server::models::view::notification::UserNotificationView;
use darve_server::models::view::task::TaskRequestView;
use darve_server::models::view::task::TaskViewForParticipant;
use darve_server::routes::tasks::TaskRequestOfferInput;
use darve_server::routes::user_auth::login_routes;
use darve_server::services::discussion_service::CreateDiscussion;
use darve_server::services::task_service::TaskRequestInput;
use helpers::post_helpers::create_fake_post;
use serde_json::json;
use std::i64;

use crate::helpers::{create_fake_login_test_user, create_login_test_user};
use community_entity::CommunityDbService;
use login_routes::LoginInput;
use middleware::ctx::Ctx;
use middleware::utils::string_utils::get_string_thing;
use wallet_entity::WalletDbService;

test_with_server!(
    create_task_request_participation,
    |server, ctx_state, config| {
        // let (server, user3, user3_pwd, _) = create_fake_login_test_user(&server).await;
        // let user_ident3 = user3.id.as_ref().unwrap().to_raw();
        // let username3 = user3.username.to_string();

        let username4 = "usnnnn4".to_string();

        // let (server, user1, _, _) = create_fake_login_test_user(&server).await;

        let (server, user0, user0_pwd, _user0_token) = create_fake_login_test_user(&server).await;
        let user_ident0 = user0.id.as_ref().unwrap().to_raw();
        let username0 = user0.username.to_string();

        ////////// user 0 creates post (user 2 creates task and user3 participates on this post for user 0 who delivers it, user4 tries to participates without enough funds)

        // create community
        let create_response = server
            .post("/api/discussions")
            .json(&CreateDiscussion {
                title: "The Community Test".to_string(),
                community_id: CommunityDbService::get_profile_community_id(
                    &user0.id.as_ref().unwrap(),
                )
                .to_raw(),
                image_uri: None,
                chat_user_ids: None,
                private_discussion_users_final: false,
            })
            .add_header("Accept", "application/json")
            .await;
        create_response.assert_status_success();
        let created = &create_response.json::<Discussion>();

        let ctx = Ctx::new(Ok(user_ident0.clone()), false);
        let community_discussion_id = created.id.clone();

        let post_name = "post title Name 1".to_string();
        let create_post = server
            .post(format!("/api/discussions/{community_discussion_id}/posts").as_str())
            .multipart(
                MultipartForm::new()
                    .add_text("title", post_name.clone())
                    .add_text("content", "contentttt"),
            )
            .add_header("Accept", "application/json")
            .await;
        let created_post = create_post.json::<Post>();
        create_post.assert_status_success();
        let post_id = created_post.id.as_ref().unwrap().to_raw();

        ////////// user 2 creates offer for user 0

        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let username2 = user2.username;

        // endow user 2
        let user2_endow_amt = 200;
        let endow_user_response = server
            .get(&format!(
                "/test/api/deposit/{}/{}",
                username2, user2_endow_amt
            ))
            .add_header("Accept", "application/json")
            .json("")
            .await;
        endow_user_response.assert_status_success();

        let user2_offer_amt: i64 = 200;
        let offer_content = "contdad".to_string();
        let task_request = server
            .post(&format!("/api/posts/{post_id}/tasks"))
            .json(&TaskRequestInput {
                offer_amount: Some(user2_offer_amt as u64),
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

        assert_eq!(created_task.id.unwrap(), task.id);

        assert_eq!(offer0.amount.clone(), user2_offer_amt as i64);
        assert_eq!(task.created_by.username, username2);
        // assert_eq!(task.to_user.clone().unwrap().username, username0);
        assert_eq!(task.donors.len(), 1);
        assert_eq!(offer0.user.username, username2);

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
        let user3_offer_amt: i64 = 100;
        let endow_user_response = server
            .get(&format!(
                "/test/api/deposit/{}/{}",
                user3.username, user3_endow_amt
            ))
            .add_header("Accept", "application/json")
            .json("")
            .await;
        endow_user_response.assert_status_success();

        let participate_response = server
            .post(format!("/api/tasks/{}/donor", task.id.to_raw()).as_str())
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
        let user3_offer_amt: i64 = 100;
        let participate_response = server
            .post(format!("/api/tasks/{}/donor", task.id.to_raw()).as_str())
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
            .post(format!("/api/tasks/{}/donor", task.id.to_raw()).as_str())
            .json(&TaskRequestOfferInput { amount: 100 })
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
        let received_post_tasks = received_post_tasks_req.json::<Vec<TaskViewForParticipant>>();

        assert_eq!(received_post_tasks.len(), 1);
        let received_task = received_post_tasks.get(0).unwrap();
        // assert_eq!(received_task.status, TaskStatus::Requested.to_string());
        // assert_eq!(received_task.deliverables.is_none(), true);

        // accept received task
        let accept_response = server
            .post(format!("/api/tasks/{}/accept", received_task.id.to_raw()).as_str())
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();

        // check task is accepted
        let received_post_tasks_req = server
            .get("/api/tasks/received")
            .add_header("Accept", "application/json")
            .await;

        received_post_tasks_req.assert_status_success();
        let received_post_tasks = received_post_tasks_req.json::<Vec<TaskViewForParticipant>>();

        assert_eq!(received_post_tasks.len(), 1);
        // let received_task = received_post_tasks.get(0).unwrap();
        // assert_eq!(received_task.status, TaskStatus::Accepted.to_string());

        //////// deliver task

        let disc_id =
            DiscussionDbService::get_profile_discussion_id(&user0.id.as_ref().unwrap()).to_raw();
        // create post on own profile for task delivery
        let post_name = "delivery post".to_string();
        let create_post = server
            .post(&format!("/api/discussions/{disc_id}/posts"))
            .multipart(
                MultipartForm::new()
                    .add_text("title", post_name.clone())
                    .add_text("content", "delivery contentttt"),
            )
            .add_header("Accept", "application/json")
            .await;
        let created_post = create_post.json::<Post>();
        create_post.assert_status_success();
        let delivery_post_id = created_post.id.as_ref().unwrap().to_raw();
        println!("DEL POST={}", delivery_post_id.clone());

        // deliver task
        let delivery_req = server
            .post(format!("/api/tasks/{}/deliver", received_task.id.to_raw()).as_str())
            .json(&json!({"post_id": delivery_post_id}))
            .await;
        delivery_req.assert_status_success();

        let received_post_tasks_req = server
            .get("/api/tasks/received")
            .add_header("Accept", "application/json")
            .await;

        received_post_tasks_req.assert_status_success();
        let received_post_tasks = received_post_tasks_req.json::<Vec<TaskViewForParticipant>>();
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
        let received_notifications = notif_history_req.json::<Vec<UserNotificationView>>();

        assert_eq!(received_notifications.len(), 2);

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

        let transactions = &transaction_history_response.json::<Vec<CurrencyTransactionView>>();
        assert_eq!(transactions.len(), 4);

        transactions.iter().fold(i64::MAX, |prev_val, tx_v| {
            let ts = tx_v.created_at.timestamp();
            println!(
                "for {} with {} in {:?} out {:?} after tx balance={}",
                tx_v.wallet.id, tx_v.with_wallet.id, tx_v.amount_in, tx_v.amount_out, tx_v.balance
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

        let transactions = &transaction_history_response.json::<Vec<CurrencyTransactionView>>();
        assert_eq!(transactions.len(), 2);

        transactions.iter().fold(i64::MAX, |prev_val, tx_v| {
            let ts = tx_v.created_at.timestamp();
            println!(
                "for {} with {} in {:?} out {:?} after tx balance={}",
                tx_v.wallet.id, tx_v.with_wallet.id, tx_v.amount_in, tx_v.amount_out, tx_v.balance
            );
            assert_eq!(ts <= prev_val, true);
            ts
        });
    }
);

test_with_server!(get_notifications, |server, ctx_state, config| {
    let (_, _user, _password, token) = create_fake_login_test_user(&server).await;
    let (_, user1, _password, token1) = create_fake_login_test_user(&server).await;
    let discussion_id = DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());

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
    let notifications = req.json::<Vec<UserNotificationView>>();
    assert_eq!(notifications.len(), 4);
    let req = server
        .get("/api/notifications?count=1")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;

    req.assert_status_success();
    let notifications = req.json::<Vec<UserNotificationView>>();
    assert_eq!(notifications.len(), 1);
    let req = server
        .get("/api/notifications?is_read=true")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;

    req.assert_status_success();
    let notifications = req.json::<Vec<UserNotificationView>>();
    assert_eq!(notifications.len(), 0);

    let req = server
        .get("/api/notifications")
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;

    req.assert_status_success();
    let notifications = req.json::<Vec<UserNotificationView>>();
    assert_eq!(notifications.len(), 1);
});

test_with_server!(set_read_notification, |server, ctx_state, config| {
    let (_, _user, _password, token) = create_fake_login_test_user(&server).await;
    let (_, user1, _password, _token1) = create_fake_login_test_user(&server).await;
    let discussion_id = DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());
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
    let notifications = req.json::<Vec<UserNotificationView>>();
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].is_following, true);
    assert_eq!(notifications[0].is_follower, false);

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
    let notifications = req.json::<Vec<UserNotificationView>>();
    assert_eq!(notifications.len(), 1);
    let first = notifications.first().unwrap();
    assert_eq!(first.is_read, true);
});

test_with_server!(set_read_all_notifications, |server, ctx_state, config| {
    let (_, _user, _password, token) = create_fake_login_test_user(&server).await;
    let (_, user1, _password, _token1) = create_fake_login_test_user(&server).await;
    let discussion_id = DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());

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
    let notifications = req.json::<Vec<UserNotificationView>>();
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
    let notifications = req.json::<Vec<UserNotificationView>>();
    assert_eq!(notifications.len(), 5);
});

test_with_server!(get_count_of_notifications, |server, ctx_state, config| {
    let (_, _user, _password, token) = create_fake_login_test_user(&server).await;
    let (_, user1, _password, _token1) = create_fake_login_test_user(&server).await;
    let discussion_id = DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());
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
