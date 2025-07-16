mod helpers;

use std::time::Duration;

use crate::helpers::create_fake_login_test_user;
use darve_server::{
    database::client::Db,
    entities::{
        community::discussion_entity::DiscussionDbService,
        task::task_request_entity::{TaskRequest, TaskRequestStatus},
        wallet::wallet_entity::{CurrencySymbol, WalletDbService},
    },
    jobs,
    middleware::{ctx::Ctx, utils::string_utils::get_str_thing},
};

use fake::{faker, Fake};
use helpers::post_helpers::create_fake_post;
use serde::Deserialize;
use serde_json::json;
use surrealdb::sql::Thing;

#[derive(Debug, Deserialize)]
struct TaskView {
    pub balance: i64,
    pub status: TaskRequestStatus,
}

#[allow(dead_code)]
async fn get_task_view(task_thing: Thing, db: &Db) -> TaskView {
    let mut res = db
        .query("SELECT *, wallet_id.transaction_head.USD.balance as balance FROM $id;")
        .bind(("id", task_thing.clone()))
        .await
        .unwrap();

    res.take::<Option<TaskView>>(0).unwrap().unwrap()
}

test_with_server!(
    one_donor_and_all_users_has_delivered,
    |server, state, config| {
        let _task_handle = jobs::task_payment::run(state.clone(), Duration::from_secs(2)).await;
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user0.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc, None, None).await;

        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user0.id.as_ref().unwrap().to_raw(),
                1000
            ))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();

        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(10),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
                "delivery_period": 1,
            }))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user1.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc, None, None).await;

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": post.id }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_success();
        let (server, user2, _, token2) = create_fake_login_test_user(&server).await;
        let response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token2))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user2.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc, None, None).await;

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": post.id }))
            .add_header("Cookie", format!("jwt={}", token2))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_success();

        let (server, user3, _, token3) = create_fake_login_test_user(&server).await;
        let response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token3))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user3.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc, None, None).await;

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": post.id }))
            .add_header("Cookie", format!("jwt={}", token3))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_success();
        let task_thing = get_str_thing(&task_id).unwrap();
        let _ = state
            .db
            .client
            .query("UPDATE $id SET delivery_period=0,acceptance_period=0;")
            .bind(("id", task_thing.clone()))
            .await;
        tokio::time::sleep(Duration::from_secs(10)).await;
        let wallet_service = WalletDbService {
            db: &state.db.client,
            ctx: &Ctx::new(Ok("".to_string()), false),
        };

        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                user1.id.as_ref().unwrap().id.to_raw().as_str(),
            )))
            .await
            .unwrap();

        assert_eq!(balance.balance_usd, 3);
        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                user2.id.as_ref().unwrap().id.to_raw().as_str(),
            )))
            .await
            .unwrap();

        assert_eq!(balance.balance_usd, 3);
        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                user3.id.as_ref().unwrap().id.to_raw().as_str(),
            )))
            .await
            .unwrap();
        assert_eq!(balance.balance_usd, 3);
        let task = get_task_view(task_thing, &state.db.client).await;
        assert_eq!(task.status, TaskRequestStatus::Completed);
        assert_eq!(task.balance, 1);
    }
);

test_with_server!(
    one_donor_and_two_users_have_delivered_and_one_user_has_not,
    |server, state, config| {
        let _task_handle = jobs::task_payment::run(state.clone(), Duration::from_secs(2)).await;
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user0.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc, None, None).await;

        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user0.id.as_ref().unwrap().to_raw(),
                1000
            ))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();

        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(10),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
                "delivery_period": 1,
            }))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user1.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc, None, None).await;

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": post.id }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_success();
        let (server, user2, _, token2) = create_fake_login_test_user(&server).await;
        let response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token2))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user2.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc, None, None).await;

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": post.id }))
            .add_header("Cookie", format!("jwt={}", token2))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_success();

        let (server, user3, _, token3) = create_fake_login_test_user(&server).await;
        let response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token3))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let task_thing = get_str_thing(&task_id).unwrap();
        let _ = state
            .db
            .client
            .query("UPDATE $id SET delivery_period=0,acceptance_period=0;")
            .bind(("id", task_thing.clone()))
            .await;
        tokio::time::sleep(Duration::from_secs(10)).await;
        let wallet_service = WalletDbService {
            db: &state.db.client,
            ctx: &Ctx::new(Ok("".to_string()), false),
        };

        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                user1.id.as_ref().unwrap().id.to_raw().as_str(),
            )))
            .await
            .unwrap();

        assert_eq!(balance.balance_usd, 5);
        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                user2.id.as_ref().unwrap().id.to_raw().as_str(),
            )))
            .await
            .unwrap();

        assert_eq!(balance.balance_usd, 5);
        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                user3.id.as_ref().unwrap().id.to_raw().as_str(),
            )))
            .await
            .unwrap();
        assert_eq!(balance.balance_usd, 0);
        let task = get_task_view(task_thing, &state.db.client).await;
        assert_eq!(task.status, TaskRequestStatus::Completed);
        assert_eq!(task.balance, 0);
    }
);

test_with_server!(one_donor_and_has_not_delivered, |server, state, config| {
    let _task_handle = jobs::task_payment::run(state.clone(), Duration::from_secs(2)).await;
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let disc = Thing::from((
        DiscussionDbService::get_table_name().as_ref(),
        user0.id.as_ref().unwrap().id.to_raw().as_ref(),
    ));
    let post = create_fake_post(server, &disc, None, None).await;
    let start_wallet_amount = 1000;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user0.id.as_ref().unwrap().to_raw(),
            start_wallet_amount
        ))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(10),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let task_thing = get_str_thing(&task_id).unwrap();
    let _ = state
        .db
        .client
        .query("UPDATE $id SET delivery_period=0,acceptance_period=0;")
        .bind(("id", task_thing.clone()))
        .await;
    tokio::time::sleep(Duration::from_secs(10)).await;

    let wallet_service = WalletDbService {
        db: &state.db.client,
        ctx: &Ctx::new(Ok("".to_string()), false),
    };

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            user1.id.as_ref().unwrap().id.to_raw().as_str(),
        )))
        .await
        .unwrap();

    assert_eq!(balance.balance_usd, 0);

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            user0.id.as_ref().unwrap().id.to_raw().as_str(),
        )))
        .await
        .unwrap();
    assert_eq!(balance.balance_usd, start_wallet_amount);
    let task = get_task_view(task_thing, &state.db.client).await;
    assert_eq!(task.status, TaskRequestStatus::Completed);
    assert_eq!(task.balance, 0);
});

test_with_server!(two_donor_and_has_not_delivered, |server, state, config| {
    let _task_handle = jobs::task_payment::run(state.clone(), Duration::from_secs(2)).await;
    let (server, donor0, _, token0) = create_fake_login_test_user(&server).await;
    let disc = Thing::from((
        DiscussionDbService::get_table_name().as_ref(),
        donor0.id.as_ref().unwrap().id.to_raw().as_ref(),
    ));
    let post = create_fake_post(server, &disc, None, None).await;
    let donor0_amount = 1000;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            donor0.id.as_ref().unwrap().to_raw(),
            donor0_amount
        ))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(10),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

    let (server, donor1, _, donor1_token) = create_fake_login_test_user(&server).await;
    let donor1_amount = 100;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            donor1.id.as_ref().unwrap().to_raw(),
            donor1_amount
        ))
        .add_header("Cookie", format!("jwt={}", donor1_token))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();
    let participate_response = server
        .post(&format!("/api/tasks/{}/donor", task_id))
        .json(&json!({
            "amount": 100,
            "currency": CurrencySymbol::USD.to_string(),
        }))
        .add_header("Accept", "application/json")
        .await;
    participate_response.assert_status_success();

    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let task_thing = get_str_thing(&task_id).unwrap();
    let _ = state
        .db
        .client
        .query("UPDATE $id SET delivery_period=0,acceptance_period=0;")
        .bind(("id", task_thing.clone()))
        .await;
    tokio::time::sleep(Duration::from_secs(10)).await;

    let wallet_service = WalletDbService {
        db: &state.db.client,
        ctx: &Ctx::new(Ok("".to_string()), false),
    };

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            user1.id.as_ref().unwrap().id.to_raw().as_str(),
        )))
        .await
        .unwrap();

    assert_eq!(balance.balance_usd, 0);

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            donor0.id.as_ref().unwrap().id.to_raw().as_str(),
        )))
        .await
        .unwrap();
    assert_eq!(balance.balance_usd, donor0_amount);

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            donor1.id.as_ref().unwrap().id.to_raw().as_str(),
        )))
        .await
        .unwrap();
    assert_eq!(balance.balance_usd, donor1_amount);
    let task = get_task_view(task_thing, &state.db.client).await;
    assert_eq!(task.status, TaskRequestStatus::Completed);
    assert_eq!(task.balance, 0);
});

test_with_server!(five_donor_and_has_not_delivered, |server, state, config| {
    let _task_handle = jobs::task_payment::run(state.clone(), Duration::from_secs(2)).await;
    let (server, donor0, _, token0) = create_fake_login_test_user(&server).await;
    let disc = Thing::from((
        DiscussionDbService::get_table_name().as_ref(),
        donor0.id.as_ref().unwrap().id.to_raw().as_ref(),
    ));
    let post = create_fake_post(server, &disc, None, None).await;
    let donor0_amount = 1000;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            donor0.id.as_ref().unwrap().to_raw(),
            donor0_amount
        ))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(10),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

    let (server, donor1, _, _) = create_fake_login_test_user(&server).await;
    let donor1_amount = 100;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            donor1.id.as_ref().unwrap().to_raw(),
            donor1_amount
        ))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();
    let participate_response = server
        .post(&format!("/api/tasks/{}/donor", task_id))
        .json(&json!({
            "amount": donor1_amount,
            "currency": CurrencySymbol::USD.to_string(),
        }))
        .add_header("Accept", "application/json")
        .await;
    participate_response.assert_status_success();

    let (server, donor2, _, donor2_token) = create_fake_login_test_user(&server).await;
    let donor2_amount = 100;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            donor2.id.as_ref().unwrap().to_raw(),
            donor2_amount
        ))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();
    let participate_response = server
        .post(&format!("/api/tasks/{}/donor", task_id))
        .json(&json!({
            "amount": donor2_amount,
            "currency": CurrencySymbol::USD.to_string(),
        }))
        .add_header("Cookie", format!("jwt={}", donor2_token))
        .add_header("Accept", "application/json")
        .await;
    participate_response.assert_status_success();

    let (server, donor3, _, donor3_token) = create_fake_login_test_user(&server).await;
    let donor3_amount = 100;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            donor3.id.as_ref().unwrap().to_raw(),
            donor3_amount
        ))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();
    let participate_response = server
        .post(&format!("/api/tasks/{}/donor", task_id))
        .json(&json!({
            "amount": donor3_amount,
            "currency": CurrencySymbol::USD.to_string(),
        }))
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", donor3_token))
        .await;
    participate_response.assert_status_success();

    let (server, donor4, _, donor4_token) = create_fake_login_test_user(&server).await;
    let donor4_amount = 100;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            donor4.id.as_ref().unwrap().to_raw(),
            donor4_amount
        ))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();
    let participate_response = server
        .post(&format!("/api/tasks/{}/donor", task_id))
        .json(&json!({
            "amount": donor4_amount,
            "currency": CurrencySymbol::USD.to_string(),
        }))
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", donor4_token))
        .await;
    participate_response.assert_status_success();

    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let task_thing = get_str_thing(&task_id).unwrap();
    let _ = state
        .db
        .client
        .query("UPDATE $id SET delivery_period=0,acceptance_period=0;")
        .bind(("id", task_thing.clone()))
        .await;

    tokio::time::sleep(Duration::from_secs(10)).await;

    let wallet_service = WalletDbService {
        db: &state.db.client,
        ctx: &Ctx::new(Ok("".to_string()), false),
    };

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            user1.id.as_ref().unwrap().id.to_raw().as_str(),
        )))
        .await
        .unwrap();

    assert_eq!(balance.balance_usd, 0);

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            donor0.id.as_ref().unwrap().id.to_raw().as_str(),
        )))
        .await
        .unwrap();
    assert_eq!(balance.balance_usd, donor0_amount);

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            donor1.id.as_ref().unwrap().id.to_raw().as_str(),
        )))
        .await
        .unwrap();
    assert_eq!(balance.balance_usd, donor1_amount);

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            donor2.id.as_ref().unwrap().id.to_raw().as_str(),
        )))
        .await
        .unwrap();
    assert_eq!(balance.balance_usd, donor2_amount);

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            donor3.id.as_ref().unwrap().id.to_raw().as_str(),
        )))
        .await
        .unwrap();
    assert_eq!(balance.balance_usd, donor3_amount);

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            donor4.id.as_ref().unwrap().id.to_raw().as_str(),
        )))
        .await
        .unwrap();
    assert_eq!(balance.balance_usd, donor4_amount);
    let task = get_task_view(task_thing, &state.db.client).await;
    assert_eq!(task.status, TaskRequestStatus::Completed);
    assert_eq!(task.balance, 0);
});

test_with_server!(
    two_donor_and_one_user_has_delivered,
    |server, state, config| {
        let _task_handle = jobs::task_payment::run(state.clone(), Duration::from_secs(2)).await;
        let (server, donor0, _, token0) = create_fake_login_test_user(&server).await;
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            donor0.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc, None, None).await;
        let donor0_amount = 1000;
        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                donor0.id.as_ref().unwrap().to_raw(),
                donor0_amount
            ))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();
        let donor0_task_amount = 100;
        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": donor0_task_amount,
                "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
                "delivery_period": 1,
            }))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();
        let (server, donor1, _, _) = create_fake_login_test_user(&server).await;
        let donor1_amount = 100;
        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                donor1.id.as_ref().unwrap().to_raw(),
                donor1_amount
            ))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        let donor1_task_amount = 100;
        endow_user_response.assert_status_success();
        let participate_response = server
            .post(&format!("/api/tasks/{}/donor", task_id))
            .json(&json!({
                "amount": donor1_task_amount,
                "currency": CurrencySymbol::USD.to_string(),
            }))
            .add_header("Accept", "application/json")
            .await;
        participate_response.assert_status_success();

        let (server, user1, _, user1_token) = create_fake_login_test_user(&server).await;
        let response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", user1_token))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user1.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc, None, None).await;

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": post.id }))
            .add_header("Cookie", format!("jwt={}", user1_token))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_success();
        let task_thing = get_str_thing(&task_id).unwrap();
        let _ = state
            .db
            .client
            .query("UPDATE $id SET delivery_period=0,acceptance_period=0;")
            .bind(("id", task_thing.clone()))
            .await;
        tokio::time::sleep(Duration::from_secs(10)).await;

        let wallet_service = WalletDbService {
            db: &state.db.client,
            ctx: &Ctx::new(Ok("".to_string()), false),
        };

        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                user1.id.as_ref().unwrap().id.to_raw().as_str(),
            )))
            .await
            .unwrap();

        assert_eq!(balance.balance_usd, donor0_task_amount + donor1_task_amount);

        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                donor0.id.as_ref().unwrap().id.to_raw().as_str(),
            )))
            .await
            .unwrap();
        assert_eq!(balance.balance_usd, donor0_amount - donor0_task_amount);

        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                donor1.id.as_ref().unwrap().id.to_raw().as_str(),
            )))
            .await
            .unwrap();
        assert_eq!(balance.balance_usd, donor1_amount - donor1_task_amount);
        let task = get_task_view(task_thing, &state.db.client).await;
        assert_eq!(task.status, TaskRequestStatus::Completed);
        assert_eq!(task.balance, 0);
    }
);
