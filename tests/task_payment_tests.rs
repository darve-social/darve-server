mod helpers;

use std::time::Duration;

use crate::helpers::create_fake_login_test_user;
use darve_server::{
    database::client::Db,
    entities::{
        community::{
            community_entity::CommunityDbService,
            discussion_entity::{Discussion, DiscussionDbService},
        },
        task::task_request_entity::{TaskRequest, TaskRequestStatus},
        wallet::wallet_entity::{CurrencySymbol, WalletDbService},
    },
    jobs,
    middleware::{ctx::Ctx, utils::string_utils::get_str_thing},
    models::view::task::TaskRequestView,
    services::discussion_service::CreateDiscussion,
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

#[allow(dead_code)]
async fn wait_for(task_thing: Thing, db: &Db) {
    let _ = db
        .query("UPDATE $id SET due_at=time::now();")
        .bind(("id", task_thing.clone()))
        .await;
    tokio::time::sleep(Duration::from_secs(10)).await;
}

test_with_server!(
    one_donor_and_all_users_has_delivered,
    |server, state, config| {
        let _task_handle = jobs::task_payment::run(state.clone(), Duration::from_secs(2)).await;
        let (server, participant1, _, p1_token) = create_fake_login_test_user(&server).await;
        let p1_disc =
            DiscussionDbService::get_profile_discussion_id(&participant1.id.as_ref().unwrap());
        let p1_post = create_fake_post(server, &p1_disc, None, None).await;

        let (server, participant2, _, p2_token) = create_fake_login_test_user(&server).await;
        let p2_disc =
            DiscussionDbService::get_profile_discussion_id(&participant2.id.as_ref().unwrap());
        let p2_post = create_fake_post(server, &p2_disc, None, None).await;

        let (server, participant3, _, p3_token) = create_fake_login_test_user(&server).await;
        let p3_disc =
            DiscussionDbService::get_profile_discussion_id(&participant3.id.as_ref().unwrap());
        let p3_post = create_fake_post(server, &p3_disc, None, None).await;

        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;

        let disc_res = server
            .post("/api/discussions")
            .json(&CreateDiscussion {
                community_id: CommunityDbService::get_profile_community_id(
                    user0.id.as_ref().unwrap(),
                )
                .to_raw(),
                title: "Hello".to_string(),
                image_uri: None,
                chat_user_ids: Some(vec![
                    participant1.id.as_ref().unwrap().to_raw(),
                    participant2.id.as_ref().unwrap().to_raw(),
                    participant3.id.as_ref().unwrap().to_raw(),
                ]),
                private_discussion_users_final: true,
            })
            .await;
        let disc = disc_res.json::<Discussion>().id;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user0.username, 1000))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();

        let task_request = server
            .post(format!("/api/discussions/{}/tasks", disc.to_raw()).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
                "delivery_period": 1,
            }))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

        let response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", p1_token))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": p1_post.id }))
            .add_header("Cookie", format!("jwt={}", p1_token))
            .add_header("Accept", "application/json")
            .await;

        delivered_response.assert_status_success();

        let response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", p2_token))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": p2_post.id }))
            .add_header("Cookie", format!("jwt={}", p2_token))
            .add_header("Accept", "application/json")
            .await;

        delivered_response.assert_status_success();

        let response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", p3_token))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": p3_post.id }))
            .add_header("Cookie", format!("jwt={}", p3_token))
            .add_header("Accept", "application/json")
            .await;

        delivered_response.assert_status_success();

        let task_thing = get_str_thing(&task_id).unwrap();
        wait_for(task_thing.clone(), &state.db.client).await;
        let wallet_service = WalletDbService {
            db: &state.db.client,
            ctx: &Ctx::new(Ok("".to_string()), false),
        };

        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                participant1.id.as_ref().unwrap().id.to_raw().as_str(),
            )))
            .await
            .unwrap();

        assert_eq!(balance.balance_usd, 33);
        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                participant2.id.as_ref().unwrap().id.to_raw().as_str(),
            )))
            .await
            .unwrap();

        assert_eq!(balance.balance_usd, 33);
        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                participant3.id.as_ref().unwrap().id.to_raw().as_str(),
            )))
            .await
            .unwrap();
        assert_eq!(balance.balance_usd, 33);
        let task_thing = get_str_thing(&task_id).unwrap();
        let task = get_task_view(task_thing, &state.db.client).await;
        assert_eq!(task.status, TaskRequestStatus::Completed);
        assert_eq!(task.balance, 1);
    }
);

test_with_server!(
    one_donor_and_two_users_have_delivered_and_one_user_has_not,
    |server, state, config| {
        let _task_handle = jobs::task_payment::run(state.clone(), Duration::from_secs(2)).await;
        let (server, participant1, _, p1_token) = create_fake_login_test_user(&server).await;
        let p1_disc =
            DiscussionDbService::get_profile_discussion_id(&participant1.id.as_ref().unwrap());
        let p1_post = create_fake_post(server, &p1_disc, None, None).await;

        let (server, participant2, _, p2_token) = create_fake_login_test_user(&server).await;
        let p2_disc =
            DiscussionDbService::get_profile_discussion_id(&participant2.id.as_ref().unwrap());
        let p2_post = create_fake_post(server, &p2_disc, None, None).await;

        let (server, participant3, _, _) = create_fake_login_test_user(&server).await;
        let p3_disc =
            DiscussionDbService::get_profile_discussion_id(&participant3.id.as_ref().unwrap());
        let _p3_post = create_fake_post(server, &p3_disc, None, None).await;

        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;

        let disc_res = server
            .post("/api/discussions")
            .json(&CreateDiscussion {
                community_id: CommunityDbService::get_profile_community_id(
                    user0.id.as_ref().unwrap(),
                )
                .to_raw(),
                title: "Hello".to_string(),
                image_uri: None,
                chat_user_ids: Some(vec![
                    participant1.id.as_ref().unwrap().to_raw(),
                    participant2.id.as_ref().unwrap().to_raw(),
                    participant3.id.as_ref().unwrap().to_raw(),
                ]),
                private_discussion_users_final: true,
            })
            .await;
        let disc = disc_res.json::<Discussion>().id;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user0.username, 1000))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();

        let task_request = server
            .post(format!("/api/discussions/{}/tasks", disc.to_raw()).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
                "delivery_period": 1,
            }))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

        let response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", p1_token))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": p1_post.id }))
            .add_header("Cookie", format!("jwt={}", p1_token))
            .add_header("Accept", "application/json")
            .await;

        delivered_response.assert_status_success();

        let response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", p2_token))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": p2_post.id }))
            .add_header("Cookie", format!("jwt={}", p2_token))
            .add_header("Accept", "application/json")
            .await;

        delivered_response.assert_status_success();

        let task_thing = get_str_thing(&task_id).unwrap();
        wait_for(task_thing.clone(), &state.db.client).await;
        let wallet_service = WalletDbService {
            db: &state.db.client,
            ctx: &Ctx::new(Ok("".to_string()), false),
        };

        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                participant1.id.as_ref().unwrap().id.to_raw().as_str(),
            )))
            .await
            .unwrap();

        assert_eq!(balance.balance_usd, 50);
        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                participant2.id.as_ref().unwrap().id.to_raw().as_str(),
            )))
            .await
            .unwrap();

        assert_eq!(balance.balance_usd, 50);
        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                participant3.id.as_ref().unwrap().id.to_raw().as_str(),
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
    let (server, participant, _, ptoken) = create_fake_login_test_user(&server).await;
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let disc = DiscussionDbService::get_profile_discussion_id(&user0.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc, None, None).await;
    let start_wallet_amount = 1000;
    let endow_user_response = server
        .get(&format!(
            "/test/api/deposit/{}/{}",
            user0.username, start_wallet_amount
        ))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participant": Some(participant.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

    let response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", ptoken))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let task_thing = get_str_thing(&task_id).unwrap();
    wait_for(task_thing.clone(), &state.db.client).await;

    let wallet_service = WalletDbService {
        db: &state.db.client,
        ctx: &Ctx::new(Ok("".to_string()), false),
    };

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            participant.id.as_ref().unwrap().id.to_raw().as_str(),
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
    let (server, participant, _, ptoken) = create_fake_login_test_user(&server).await;
    let (server, donor0, _, token0) = create_fake_login_test_user(&server).await;
    let disc = DiscussionDbService::get_profile_discussion_id(&donor0.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc, None, None).await;
    let donor0_amount = 1000;
    let endow_user_response = server
        .get(&format!(
            "/test/api/deposit/{}/{}",
            donor0.username, donor0_amount
        ))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participant": Some(participant.id.as_ref().unwrap().to_raw()),
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
            "/test/api/deposit/{}/{}",
            donor1.username, donor1_amount
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

    let response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", ptoken))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let task_thing = get_str_thing(&task_id).unwrap();
    wait_for(task_thing.clone(), &state.db.client).await;

    let wallet_service = WalletDbService {
        db: &state.db.client,
        ctx: &Ctx::new(Ok("".to_string()), false),
    };

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            participant.id.as_ref().unwrap().id.to_raw().as_str(),
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
    let (server, participant, _, ptoken) = create_fake_login_test_user(&server).await;
    let _task_handle = jobs::task_payment::run(state.clone(), Duration::from_secs(2)).await;
    let (server, donor0, _, token0) = create_fake_login_test_user(&server).await;
    let disc = DiscussionDbService::get_profile_discussion_id(&donor0.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc, None, None).await;
    let donor0_amount = 1000;
    let endow_user_response = server
        .get(&format!(
            "/test/api/deposit/{}/{}",
            donor0.username, donor0_amount
        ))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participant": Some(participant.id.as_ref().unwrap().to_raw()),
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
            "/test/api/deposit/{}/{}",
            donor1.username, donor1_amount
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
            "/test/api/deposit/{}/{}",
            donor2.username, donor2_amount
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
            "/test/api/deposit/{}/{}",
            donor3.username, donor3_amount
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
            "/test/api/deposit/{}/{}",
            donor4.username, donor4_amount
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

    let response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", ptoken))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let task_thing = get_str_thing(&task_id).unwrap();
    wait_for(task_thing.clone(), &state.db.client).await;

    let wallet_service = WalletDbService {
        db: &state.db.client,
        ctx: &Ctx::new(Ok("".to_string()), false),
    };

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            participant.id.as_ref().unwrap().id.to_raw().as_str(),
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
        let (server, participant, _, ptoken) = create_fake_login_test_user(&server).await;
        let p_disc =
            DiscussionDbService::get_profile_discussion_id(&participant.id.as_ref().unwrap());
        let p_post = create_fake_post(server, &p_disc, None, None).await;

        let (server, donor0, _, token0) = create_fake_login_test_user(&server).await;
        let disc = DiscussionDbService::get_profile_discussion_id(&donor0.id.as_ref().unwrap());
        let post = create_fake_post(server, &disc, None, None).await;
        let donor0_amount = 1000;
        let endow_user_response = server
            .get(&format!(
                "/test/api/deposit/{}/{}",
                donor0.username, donor0_amount
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
                "participant": Some(participant.id.as_ref().unwrap().to_raw()),
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
                "/test/api/deposit/{}/{}",
                donor1.username, donor1_amount
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

        let response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", ptoken))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": p_post.id }))
            .add_header("Cookie", format!("jwt={}", ptoken))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_success();
        let task_thing = get_str_thing(&task_id).unwrap();
        wait_for(task_thing.clone(), &state.db.client).await;

        let wallet_service = WalletDbService {
            db: &state.db.client,
            ctx: &Ctx::new(Ok("".to_string()), false),
        };

        let balance = wallet_service
            .get_balance(&Thing::from((
                "wallet",
                participant.id.as_ref().unwrap().id.to_raw().as_str(),
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

test_with_server!(immediately_refund_on_reject, |server, state, config| {
    let (server, participant, _, token) = create_fake_login_test_user(&server).await;

    let (server, donor0, _, token0) = create_fake_login_test_user(&server).await;
    let disc = DiscussionDbService::get_profile_discussion_id(&donor0.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc, None, None).await;
    let donor0_amount = 1000;
    let endow_user_response = server
        .get(&format!(
            "/test/api/deposit/{}/{}",
            donor0.username, donor0_amount
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
            "participant": Some(participant.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();

    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

    let response = server
        .post(&format!("/api/tasks/{}/reject", task_id))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    let wallet_service = WalletDbService {
        db: &state.db.client,
        ctx: &Ctx::new(Ok("".to_string()), false),
    };

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            participant.id.as_ref().unwrap().id.to_raw().as_str(),
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
});

test_with_server!(immediately_refund_on_reject_1, |server, state, config| {
    let (server, participant, _, token) = create_fake_login_test_user(&server).await;
    let (server, donor0, _, token0) = create_fake_login_test_user(&server).await;
    let comm_id = CommunityDbService::get_profile_community_id(&donor0.id.as_ref().unwrap());

    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.to_raw(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: Some(
                [
                    donor0.id.as_ref().unwrap().to_raw(),
                    participant.id.as_ref().unwrap().to_raw(),
                ]
                .to_vec(),
            ),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    let disc = create_response.json::<Discussion>();

    let task_request = server
        .post(format!("/api/discussions/{}/tasks", disc.id.to_raw()).as_str())
        .json(&json!({
            "participant": Some(participant.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();

    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

    let response = server
        .post(&format!("/api/tasks/{}/reject", task_id))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    let task_request = server
        .get(format!("/api/tasks/{}", task_id).as_str())
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task = task_request.json::<TaskRequestView>();

    assert_eq!(task.status, TaskRequestStatus::Completed);
});
