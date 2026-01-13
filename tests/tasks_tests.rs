mod helpers;

use std::str::FromStr;

use crate::helpers::{create_fake_login_test_user, task_helpers};
use chrono::Utc;
use darve_server::{
    entities::{
        community::{
            community_entity::CommunityDbService,
            discussion_entity::{Discussion, DiscussionDbService},
            post_entity::{Post, PostDbService},
        },
        tag::SystemTags,
        task_request::TaskRequestEntity,
        task_request_user::TaskParticipantStatus,
        wallet::wallet_entity::{CurrencySymbol, WalletDbService},
    },
    interfaces::repositories::tags::TagsRepositoryInterface,
    middleware::{ctx::Ctx, utils::db_utils::Pagination},
    models::view::{
        access::PostAccessView,
        post::PostView,
        task::{TaskRequestView, TaskViewForParticipant},
    },
    services::discussion_service::CreateDiscussion,
};
use surrealdb::sql::Thing;

use fake::{faker, Fake};
use helpers::post_helpers::create_fake_post;
use reqwest::StatusCode;
use serde_json::json;

test_with_server!(created_closed_task_request, |server, ctx_state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_request = server
        .get("/api/tasks/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let tasks = task_request.json::<Vec<TaskViewForParticipant>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert!(first.end_at > Utc::now());
    assert_eq!(first.total_amount, 100);
    assert_eq!(first.participants.len(), 1);
    let task_user = first.participants.first().unwrap();
    assert_eq!(task_user.user.id, user0.id.as_ref().unwrap().clone());
    assert_eq!(task_user.status, TaskParticipantStatus::Requested);
    let task_request = server
        .get("/api/tasks/given")
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let tasks = task_request.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert_eq!(first.donors.len(), 1);
    let participator = first.donors.first().unwrap();
    assert_eq!(participator.user.id, user1.id.as_ref().unwrap().clone());
    assert_eq!(participator.amount, 100);
});

test_with_server!(accepted_closed_task_request, |server, ctx_state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();
    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequestEntity>().id;

    let accept_response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_success();

    let task_request = server
        .get("/api/tasks/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let tasks = task_request.json::<Vec<TaskViewForParticipant>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert_eq!(first.participants.len(), 1);
    let task_user = first.participants.first().unwrap();
    assert_eq!(task_user.user.id, user0.id.as_ref().unwrap().clone());
    assert_eq!(task_user.status, TaskParticipantStatus::Accepted);
});

test_with_server!(
    deny_to_create_public_task_in_public_post,
    |server, ctx_state, config| {
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
        let post = create_fake_post(server, &disc_id, None, None).await;

        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_forbidden();
    }
);

test_with_server!(accepted_opened_task_request, |server, ctx_state, config| {
    let (server, _user, _, token) = create_fake_login_test_user(&server).await;
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();
    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequestEntity>().id;

    let accept_response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_forbidden();

    let accept_response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_success();

    let accept_response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_forbidden();

    let task_request = server
        .get("/api/tasks/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let tasks = task_request.json::<Vec<TaskViewForParticipant>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert_eq!(first.participants.len(), 1);
    let task_users = &first.participants;
    assert_eq!(task_users.len(), 1);
    let task_user0 = task_users
        .iter()
        .find(|t| &t.user.id == user0.id.as_ref().unwrap())
        .unwrap();
    assert_eq!(task_user0.user.id, user0.id.as_ref().unwrap().clone());
    assert_eq!(task_user0.status, TaskParticipantStatus::Accepted);
});

test_with_server!(
    try_to_accept_task_request_after_rejected,
    |server, ctx_state, config| {
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();
        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                "participants": vec![user0.id.as_ref().unwrap().to_raw()],
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequestEntity>().id;

        let accept_response = server
            .post(&format!("/api/tasks/{}/reject", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();
        let accept_response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_failure();
        assert!(accept_response.text().contains("Forbidden"));
    }
);

test_with_server!(
    try_to_accept_task_request_after_delivered,
    |server, ctx_state, config| {
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();
        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                "participants": vec![user0.id.as_ref().unwrap().to_raw()],
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequestEntity>().id;

        let accept_response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();

        let _ = task_helpers::success_deliver_task(&server, &task_id, &token0)
            .await
            .unwrap();

        let accept_response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_failure();
        assert!(accept_response.text().contains("Forbidden"));
    }
);

test_with_server!(
    try_to_accept_task_request_participator,
    |server, ctx_state, config| {
        let (server, _user, _, _token) = create_fake_login_test_user(&server).await;
        let (server, user0, _, _token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();
        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                 "participants": vec![user0.id.as_ref().unwrap().to_raw()],
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequestEntity>().id;

        let accept_response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_failure();
        assert!(accept_response.text().contains("Forbidden"));
    }
);

test_with_server!(
    try_to_accept_closed_task_request,
    |server, ctx_state, config| {
        let (server, _user, _, token) = create_fake_login_test_user(&server).await;
        let (server, user0, _, _token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();
        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                "participants": vec![user0.id.as_ref().unwrap().to_raw()],
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequestEntity>().id;

        let accept_response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_failure();
        assert!(accept_response.text().contains("Forbidden"));
    }
);

test_with_server!(rejected_closed_task_request, |server, ctx_state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();
    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequestEntity>().id;

    let accept_response = server
        .post(&format!("/api/tasks/{}/reject", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_success();
});

test_with_server!(rejected_opened_task_request, |server, ctx_state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());

    let post = create_fake_post(server, &disc_id, None, None).await;
    let endow_user_response = server
        .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();
    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequestEntity>().id;

    let accept_response = server
        .post(&format!("/api/tasks/{}/reject", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_success();
    let task_request = server
        .get("/api/tasks/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let tasks = task_request.json::<Vec<TaskViewForParticipant>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert_eq!(first.participants.len(), 1);
    let task_user = first.participants.first().unwrap();
    assert_eq!(task_user.user.id, user0.id.as_ref().unwrap().clone());
    assert_eq!(task_user.status, TaskParticipantStatus::Rejected);
});

test_with_server!(
    try_to_reject_task_request_after_delivered,
    |server, ctx_state, config| {
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();
        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequestEntity>().id;

        let accept_response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();

        let task_participant = task_helpers::success_deliver_task(&server, &task_id, &token0)
            .await
            .unwrap();

        assert!(task_participant.result.unwrap().post.is_some());
        let accept_response = server
            .post(&format!("/api/tasks/{}/reject", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_failure();
        assert!(accept_response.text().contains("Forbidden"));
    }
);

test_with_server!(
    try_to_reject_closed_ask_request_some_user,
    |server, ctx_state, config| {
        let (server, _user, _, token) = create_fake_login_test_user(&server).await;
        let (server, user0, _, _token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();
        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequestEntity>().id;

        let accept_response = server
            .post(&format!("/api/tasks/{}/reject", task_id))
            .add_header("Cookie", format!("jwt={}", token))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_failure();
        assert!(accept_response.text().contains("Forbidden"));
    }
);

test_with_server!(delivered_task_request, |server, ctx_state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();
    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequestEntity>().id;

    let accept_response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_success();

    let task_participant = task_helpers::success_deliver_task(&server, &task_id, &token0)
        .await
        .unwrap();

    assert!(task_participant.result.as_ref().unwrap().link.is_none());
    assert!(task_participant.result.as_ref().unwrap().post.is_some());
    let delivery_post = task_participant.result.unwrap().post.unwrap();

    let post_view = PostDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok(user0.id.as_ref().unwrap().to_raw()), false),
    }
    .get_view_by_id::<PostAccessView>(delivery_post.to_raw().as_str(), None)
    .await
    .unwrap();

    assert_eq!(post_view.users.len(), 0);

    let task_request = server
        .get("/api/tasks/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let tasks = task_request.json::<Vec<TaskViewForParticipant>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert_eq!(first.participants.len(), 1);
    let task_user = first.participants.first().unwrap();
    assert_eq!(task_user.user.id, user0.id.as_ref().unwrap().clone());
    assert_eq!(task_user.status, TaskParticipantStatus::Delivered);
});

test_with_server!(
    delivered_task_in_private_dics,
    |server, ctx_state, config| {
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let comm_id = CommunityDbService::get_profile_community_id(&user1.id.as_ref().unwrap());

        let create_response = server
            .post("/api/discussions")
            .json(&CreateDiscussion {
                community_id: comm_id.to_raw(),
                title: "The Discussion".to_string(),
                image_uri: None,
                chat_user_ids: Some(vec![user0.id.as_ref().unwrap().to_raw()]),
                private_discussion_users_final: false,
            })
            .add_header("Accept", "application/json")
            .await;

        let disc_id = create_response.json::<Discussion>().id;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();
        let task_request = server
            .post(format!("/api/discussions/{}/tasks", disc_id.to_raw()).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                "participants": vec![user0.id.as_ref().unwrap().to_raw()],
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequestEntity>().id;

        let accept_response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();

        let task_participant = task_helpers::success_deliver_task(&server, &task_id, &token0)
            .await
            .unwrap();
        let task_thing_id = Thing::from_str(&task_id).unwrap().id.to_raw();
        let link = task_participant.result.as_ref().unwrap().link.as_ref();
        assert!(link.is_some());
        assert!(task_participant.result.as_ref().unwrap().post.is_none());
        assert!(link.unwrap().contains(&task_thing_id));
        assert!(link
            .unwrap()
            .contains(&user0.id.as_ref().unwrap().id.to_raw()));

        let task_request = server
            .get("/api/tasks/received")
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let tasks = task_request.json::<Vec<TaskViewForParticipant>>();
        assert_eq!(tasks.len(), 1);
        let first = tasks.first().unwrap();
        assert_eq!(first.participants.len(), 1);
        let task_user = first.participants.first().unwrap();
        assert_eq!(task_user.user.id, user0.id.as_ref().unwrap().clone());
        assert_eq!(task_user.status, TaskParticipantStatus::Delivered);
    }
);

test_with_server!(
    try_to_deliver_task_request_after_rejected,
    |server, ctx_state, config| {
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();
        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequestEntity>().id;

        let response = server
            .post(&format!("/api/tasks/{}/reject", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();

        let delivered_response = task_helpers::deliver_task(&server, &task_id, &token0).await;
        delivered_response.assert_status_failure();
        assert!(delivered_response.text().contains("Forbidden"))
    }
);

test_with_server!(
    try_to_deliver_task_request_after_requested,
    |server, ctx_state, config| {
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();
        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequestEntity>().id;
        let delivered_response = task_helpers::deliver_task(&server, &task_id, &token0).await;
        delivered_response.assert_status_failure();
        assert!(delivered_response.text().contains("Forbidden"))
    }
);

test_with_server!(
    try_to_deliver_task_request_some_user,
    |server, ctx_state, config| {
        let (server, _user, _, token) = create_fake_login_test_user(&server).await;
        let (server, user0, _, _token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();
        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequestEntity>().id;

        let delivered_response = task_helpers::deliver_task(&server, &task_id, &token).await;
        delivered_response.assert_status_failure();
        assert!(delivered_response.text().contains("Forbidden"))
    }
);

test_with_server!(get_tasks, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc_id, None, None).await;

    let post1 = create_fake_post(server, &disc_id, None, None).await;
    let post2 = create_fake_post(server, &disc_id, None, None).await;
    let post3 = create_fake_post(server, &disc_id, None, None).await;
    let endow_user_response = server
        .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();
    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();

    let task_id = task_request.json::<TaskRequestEntity>().id;
    let response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post1.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user.id.as_ref().unwrap().to_raw()],
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();

    let task_id = task_request.json::<TaskRequestEntity>().id;
    let response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post2.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user.id.as_ref().unwrap().to_raw()],
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post3.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequestEntity>().id;

    let response = server
        .post(&format!("/api/tasks/{}/reject", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    let response = server
        .get("/api/tasks/received")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskViewForParticipant>>();
    assert_eq!(tasks.len(), 2);
    let response = server
        .get("/api/tasks/received")
        .add_query_param("status", "Rejected")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskViewForParticipant>>();
    assert_eq!(tasks.len(), 0);
    let response = server
        .get("/api/tasks/received?status=Accepted")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskViewForParticipant>>();
    assert_eq!(tasks.len(), 1);

    let response = server
        .get("/api/tasks/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskViewForParticipant>>();
    assert_eq!(tasks.len(), 2);
    let response = server
        .get("/api/tasks/received")
        .add_query_param("status", "Rejected")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskViewForParticipant>>();
    assert_eq!(tasks.len(), 1);
    let response = server
        .get("/api/tasks/received?status=Requested")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskViewForParticipant>>();
    assert_eq!(tasks.len(), 0);
    let response = server
        .get("/api/tasks/received?status=Accepted")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskViewForParticipant>>();
    assert_eq!(tasks.len(), 1);
});

test_with_server!(try_to_acceptance_task_expired, |server, state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
            "acceptance_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequestEntity>().id;

    let _ = state
        .db
        .client
        .query("UPDATE $id SET acceptance_period=0;")
        .bind(("id", Thing::try_from(task_id.as_str()).unwrap()))
        .await;

    let response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_failure();
    assert!(response
        .text()
        .contains("The acceptance period has expired"));
});

test_with_server!(try_to_delivery_task_expired, |server, state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc, None, None).await;

    let endow_user_response = server
        .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequestEntity>().id;

    let response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    let _ = state
        .db
        .client
        .query("UPDATE $id SET delivery_period=0, acceptance_period=0;")
        .bind(("id", Thing::try_from(task_id.as_ref()).unwrap()))
        .await;

    let response = task_helpers::deliver_task(&server, &task_id, &token0).await;
    response.assert_status_failure();
    assert!(response.text().contains("The delivery period has expired"));
});

test_with_server!(
    try_to_add_task_donor_without_balance,
    |server, state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let disc = DiscussionDbService::get_profile_discussion_id(user0.id.as_ref().unwrap());
        let post = create_fake_post(server, &disc, None, None).await;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user0.username, 1000))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();

        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "participants": vec![user.id.as_ref().unwrap().to_raw()],
                "delivery_period": 1,
            }))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequestEntity>().id;

        let wallet_service = WalletDbService {
            db: &state.db.client,
            ctx: &Ctx::new(Ok("".to_string()), false),
        };

        let (server, user1, _, _) = create_fake_login_test_user(&server).await;
        let balance = wallet_service
            .get_user_balance(&user1.id.as_ref().unwrap())
            .await
            .unwrap();
        assert_eq!(balance.balance_usd, 0);

        let participate_response = server
            .post(&format!("/api/tasks/{}/donor", task_id))
            .json(&json!({
                "amount": 100,
                "currency": CurrencySymbol::USD.to_string(),
            }))
            .add_header("Accept", "application/json")
            .await;

        participate_response.assert_status_failure();
        participate_response.assert_status(StatusCode::PAYMENT_REQUIRED);
    }
);

test_with_server!(
    try_to_add_task_donor_without_access,
    |server, state, config| {
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let comm_id =
            CommunityDbService::get_profile_community_id(&user0.id.as_ref().unwrap().clone());
        let create_response = server
            .post("/api/discussions")
            .json(&json!({
                "community_id": comm_id.to_raw(),
                "title": "The Discussion".to_string(),
                "chat_user_ids": [user2.id.as_ref().unwrap().to_raw()],
                "private_discussion_users_final": false,
            }))
            .add_header("Accept", "application/json")
            .await;

        create_response.assert_status_success();
        let created = create_response.json::<Discussion>();

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user0.username, 1000))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();

        let task_request = server
            .post(format!("/api/discussions/{}/tasks", created.id.to_raw()).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
                "delivery_period": 1,
            }))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequestEntity>().id;

        let (server, user1, _, user1_token) = create_fake_login_test_user(&server).await;
        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user1.username, 10000))
            .add_header("Cookie", format!("jwt={}", user1_token))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();

        let participate_response = server
            .post(&format!("/api/tasks/{}/donor", task_id))
            .add_header("Cookie", format!("jwt={}", user1_token))
            .json(&json!({
                "amount": 100,
                "currency": CurrencySymbol::USD.to_string(),
            }))
            .add_header("Accept", "application/json")
            .await;
        participate_response.assert_status_failure();
        participate_response.assert_status(StatusCode::FORBIDDEN);
    }
);

test_with_server!(
    try_to_create_task_in_profile_discussion,
    |server, state, config| {
        let (server, user0, _, _) = create_fake_login_test_user(&server).await;
        let disc_id =
            DiscussionDbService::get_profile_discussion_id(&user0.id.as_ref().unwrap().clone());

        let request = server
            .post(format!("/api/discussions/{}/tasks", disc_id.to_raw()).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
                "delivery_period": 1,
            }))
            .add_header("Accept", "application/json")
            .await;
        request.assert_status_failure();
        request.assert_status(StatusCode::FORBIDDEN);
    }
);

test_with_server!(try_to_to_accept_without_access, |server, state, config| {
    let (server, user2, _, _) = create_fake_login_test_user(&server).await;
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let comm_id = CommunityDbService::get_profile_community_id(&user0.id.as_ref().unwrap().clone());
    let create_response = server
        .post("/api/discussions")
        .json(&json!({
            "community_id": comm_id.to_raw(),
            "title": "The Discussion".to_string(),
            "chat_user_ids": [user2.id.as_ref().unwrap().to_raw()],
            "private_discussion_users_final": false,
        }))
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_success();
    let created = create_response.json::<Discussion>();

    let endow_user_response = server
        .get(&format!("/test/api/deposit/{}/{}", user0.username, 1000))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

    let task_request = server
        .post(format!("/api/discussions/{}/tasks", created.id.to_raw()).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequestEntity>().id;

    let (server, _, _, user1_token) = create_fake_login_test_user(&server).await;

    let participate_response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", user1_token))
        .add_header("Accept", "application/json")
        .await;
    participate_response.assert_status_failure();
    participate_response.assert_status(StatusCode::FORBIDDEN);
});

test_with_server!(get_expired_tasks, |server, state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
           "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;

    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequestEntity>().id;

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
           "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();

    let _ = state
        .db
        .client
        .query("UPDATE $id SET due_at=time::now();")
        .bind(("id", Thing::try_from(task_id).unwrap()))
        .await;

    let get_response = server
        .get("/api/tasks/received")
        .add_header("Cookie", format!("jwt={token0}"))
        .await;
    get_response.assert_status_success();
    let tasks = get_response.json::<Vec<TaskViewForParticipant>>();
    assert_eq!(tasks.len(), 2);
    let get_response = server
        .get("/api/tasks/received?is_ended=true")
        .add_header("Cookie", format!("jwt={token0}"))
        .await;
    get_response.assert_status_success();
    let tasks = get_response.json::<Vec<TaskViewForParticipant>>();
    assert_eq!(tasks.len(), 1);
});

test_with_server!(
    given_tasks_public_disc_public_post,
    |server, ctx_state, config| {
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let (server, _user2, _, token2) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();
        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();

        let given_tasks_response = server
            .get("/api/tasks/given")
            .add_header("Cookie", format!("jwt={}", token1))
            .await;

        given_tasks_response.assert_status_success();

        let tasks = given_tasks_response.json::<Vec<TaskRequestView>>();

        assert_eq!(tasks.len(), 1);
        let given_tasks_response = server
            .get("/api/tasks/given")
            .add_header("Cookie", format!("jwt={}", token0))
            .await;

        given_tasks_response.assert_status_success();

        let tasks = given_tasks_response.json::<Vec<TaskRequestView>>();

        assert_eq!(tasks.len(), 0);
        let given_tasks_response = server
            .get("/api/tasks/given")
            .add_header("Cookie", format!("jwt={}", token2))
            .await;

        given_tasks_response.assert_status_success();

        let tasks = given_tasks_response.json::<Vec<TaskRequestView>>();

        assert_eq!(tasks.len(), 0);
    }
);

test_with_server!(given_tasks_private_disc, |server, ctx_state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let comm_id = CommunityDbService::get_profile_community_id(&user1.id.as_ref().unwrap().clone());
    let create_response = server
            .post("/api/discussions")
            .json(&json!({
                "community_id": comm_id.to_raw(),
                "title": "The Discussion".to_string(),
                "chat_user_ids": [user2.id.as_ref().unwrap().to_raw(), user0.id.as_ref().unwrap().to_raw()],
                "private_discussion_users_final": false,
            }))
            .add_header("Accept", "application/json")
            .await;

    create_response.assert_status_success();
    let disc_id = create_response.json::<Discussion>().id;

    let task_request = server
        .post(format!("/api/discussions/{}/tasks", disc_id.to_raw()).as_str())
        .json(&json!({
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();

    let given_tasks_response = server
        .get("/api/tasks/given")
        .add_header("Cookie", format!("jwt={}", token0))
        .await;

    given_tasks_response.assert_status_success();
    let tasks = given_tasks_response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let given_tasks_response = server
        .get("/api/tasks/given")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    given_tasks_response.assert_status_success();
    let tasks = given_tasks_response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 0);
    let given_tasks_response = server
        .get("/api/tasks/given")
        .add_header("Cookie", format!("jwt={}", token2))
        .await;

    given_tasks_response.assert_status_success();
    let tasks = given_tasks_response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 0);

    server
        .delete(&format!("/api/discussions/{}/chat_users", disc_id.to_raw()))
        .add_header("Cookie", format!("jwt={}", token1))
        .json(&json!({ "user_ids": [user0.id.as_ref().unwrap().to_raw()]}))
        .await
        .assert_status_success();
    let given_tasks_response = server
        .get("/api/tasks/given")
        .add_header("Cookie", format!("jwt={}", token0))
        .await;

    given_tasks_response.assert_status_success();
    let tasks = given_tasks_response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 0);
});

test_with_server!(
    check_deliver_post_after_delivered,
    |server, ctx_state, config| {
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();
        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequestEntity>().id;

        let accept_response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();
        let task_participant = task_helpers::success_deliver_task(server, &task_id, &token0)
            .await
            .unwrap();
        let deliver_post = task_participant.result.unwrap().post.unwrap();

        let posts: Vec<Post> = ctx_state
            .db
            .tags
            .get_by_tag(
                SystemTags::Delivery.as_str(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: 10,
                    start: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(posts.len(), 1);
        assert_eq!(
            posts[0].id.as_ref().unwrap().to_raw(),
            deliver_post.to_raw()
        )
    }
);

test_with_server!(get_task_by_id, |server, ctx_state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, _user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;
    let comm_id = CommunityDbService::get_profile_community_id(&user2.id.as_ref().unwrap().clone());
    let create_response = server
        .post("/api/discussions")
        .json(&json!({
            "community_id": comm_id.to_raw(),
            "title": "The Discussion".to_string(),
            "chat_user_ids": [ user0.id.as_ref().unwrap().to_raw()],
            "private_discussion_users_final": false,
        }))
        .add_header("Accept", "application/json")
        .await;
    let disc_id = create_response.json::<Discussion>().id;

    let task_request = server
        .post(format!("/api/discussions/{}/tasks", disc_id.to_raw()).as_str())
        .json(&json!({
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task = task_request.json::<TaskRequestEntity>();
    let task_id = task.id;
    let get_task_response = server
        .get(&format!("/api/tasks/{}", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    get_task_response.assert_status_success();
    let task_view = get_task_response.json::<TaskRequestView>();
    assert_eq!(task_view.id, task_id);

    let get_task_response = server
        .get(&format!("/api/tasks/{}", task_id))
        .add_header("Cookie", format!("jwt={}", token2))
        .add_header("Accept", "application/json")
        .await;
    get_task_response.assert_status_success();
    let task_view = get_task_response.json::<TaskRequestView>();
    assert_eq!(task_view.id, task_id);

    let get_task_response = server
        .get(&format!("/api/tasks/{}", task_id))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    get_task_response.assert_status_forbidden();
});

test_with_server!(
    update_post_tasks_nr_on_task_creation,
    |server, ctx_state, config| {
        let (server, user0, _, _token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, _token1) = create_fake_login_test_user(&server).await;
        let (server, user2, _, _token2) = create_fake_login_test_user(&server).await;
        let disc_id =
            DiscussionDbService::get_profile_discussion_id(&user2.id.as_ref().unwrap().clone());
        let post_id = create_fake_post(server, &disc_id, None, None).await;

        server
            .post(format!("/api/posts/{}/tasks", post_id.id).as_str())
            .json(&json!({
                "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "participants": vec![user1.id.as_ref().unwrap().to_raw()],
            }))
            .add_header("Accept", "application/json")
            .await
            .assert_status_success();

        let get_post_res = server
            .get(format!("/api/posts/{}", &post_id.id).as_str())
            .await;

        get_post_res.assert_status_success();
        let post = get_post_res.json::<PostView>();

        assert_eq!(post.tasks_nr, 1);
        server
            .post(format!("/api/posts/{}/tasks", post_id.id).as_str())
            .json(&json!({
                 "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
             "participants": vec![user0.id.as_ref().unwrap().to_raw()],
            }))
            .add_header("Accept", "application/json")
            .await
            .assert_status_success();

        let get_post_res = server
            .get(format!("/api/posts/{}", &post_id.id).as_str())
            .await;

        get_post_res.assert_status_success();
        let post = get_post_res.json::<PostView>();

        assert_eq!(post.tasks_nr, 2);
    }
);
test_with_server!(
    try_to_create_task_for_youself_for_discussion,
    |server, state, config| {
        let (server, user0, _, _) = create_fake_login_test_user(&server).await;
        let disc_id =
            DiscussionDbService::get_profile_discussion_id(&user0.id.as_ref().unwrap().clone());

        let request = server
            .post(format!("/api/discussions/{}/tasks", disc_id.to_raw()).as_str())
            .json(&json!({
                 "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
             "participants": vec![user0.id.as_ref().unwrap().to_raw()],
            }))
            .add_header("Accept", "application/json")
            .await;
        request.assert_status_failure();
        request.assert_status(StatusCode::FORBIDDEN);
    }
);

test_with_server!(
    try_to_create_task_for_youself_for_post,
    |server, state, config| {
        let (server, user0, _, _) = create_fake_login_test_user(&server).await;
        let disc_id =
            DiscussionDbService::get_profile_discussion_id(&user0.id.as_ref().unwrap().clone());

        let post = create_fake_post(server, &disc_id, None, None).await;

        let request = server
            .post(&format!("/api/posts/{}/tasks", post.id))
            .json(&json!({
                "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "participants": vec![user0.id.as_ref().unwrap().to_raw()],
            }))
            .add_header("Accept", "application/json")
            .await;
        request.assert_status_failure();
        request.assert_status(StatusCode::FORBIDDEN);
    }
);
