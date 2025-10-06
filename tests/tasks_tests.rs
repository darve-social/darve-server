mod helpers;

use crate::helpers::create_fake_login_test_user;
use axum_test::multipart::MultipartForm;
use chrono::Utc;
use darve_server::{
    access::base::role::Role,
    entities::{
        community::{
            community_entity::CommunityDbService,
            discussion_entity::{Discussion, DiscussionDbService},
            post_entity::{Post, PostDbService},
        },
        tag::SystemTags,
        task::task_request_entity::TaskRequest,
        task_request_user::TaskParticipantStatus,
        wallet::wallet_entity::{CurrencySymbol, WalletDbService},
    },
    interfaces::repositories::tags::TagsRepositoryInterface,
    middleware::{
        ctx::Ctx,
        utils::{db_utils::Pagination, string_utils::get_str_thing},
    },
    models::view::{
        access::PostAccessView,
        post::PostView,
        task::{TaskRequestView, TaskViewForParticipant},
    },
};

use fake::{faker, Fake};
use helpers::post_helpers::{build_fake_post, create_fake_post};
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
            "participant": Some(user0.id.as_ref().unwrap().to_raw()),
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
            "participant": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

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
            "participant": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

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
                "participant": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

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
                "participant": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

        let accept_response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();
        let disc = DiscussionDbService::get_profile_discussion_id(user0.id.as_ref().unwrap());
        let deliver_post = server
            .post(format!("/api/discussions/{}/posts", disc.to_raw()).as_str())
            .multipart(build_fake_post(None, None))
            .add_header("Accept", "application/json")
            .add_header("Cookie", format!("jwt={}", token0))
            .await
            .json::<Post>();

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": deliver_post.id.unwrap().to_raw() }))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_success();

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
                 "participant": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

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
                "participant": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

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
            "participant": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

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
            "participant": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

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
                "participant": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

        let accept_response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();
        let disc = DiscussionDbService::get_profile_discussion_id(user0.id.as_ref().unwrap());
        let deliver_post = server
            .post(format!("/api/discussions/{}/posts", disc.to_raw()).as_str())
            .multipart(build_fake_post(None, None))
            .add_header("Accept", "application/json")
            .add_header("Cookie", format!("jwt={}", token0))
            .await
            .json::<Post>();

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": deliver_post.id.unwrap().to_raw()}))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_success();

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
                "participant": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

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
            "participant": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

    let accept_response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_success();
    let disc = DiscussionDbService::get_profile_discussion_id(user0.id.as_ref().unwrap());
    let deliver_post = server
        .post(format!("/api/discussions/{}/posts", disc.to_raw()).as_str())
        .multipart(build_fake_post(None, None))
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token0))
        .await
        .json::<Post>();

    let delivered_response = server
        .post(&format!("/api/tasks/{}/deliver", task_id))
        .json(&json!({"post_id": deliver_post.id.as_ref().unwrap().to_raw() }))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    delivered_response.assert_status_success();

    let post_view = PostDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok(user0.id.as_ref().unwrap().to_raw()), false),
    }
    .get_view_by_id::<PostAccessView>(deliver_post.id.as_ref().unwrap().to_raw().as_str(), None)
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
                "participant": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

        let response = server
            .post(&format!("/api/tasks/{}/reject", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let disc = DiscussionDbService::get_profile_discussion_id(user0.id.as_ref().unwrap());
        let deliver_post = server
            .post(format!("/api/discussions/{}/posts", disc.to_raw()).as_str())
            .multipart(build_fake_post(None, None))
            .add_header("Accept", "application/json")
            .add_header("Cookie", format!("jwt={}", token0))
            .await
            .json::<Post>();

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": deliver_post.id.unwrap().to_raw() }))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
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
                "participant": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();
        let disc = DiscussionDbService::get_profile_discussion_id(user0.id.as_ref().unwrap());
        let deliver_post = server
            .post(format!("/api/discussions/{}/posts", disc.to_raw()).as_str())
            .multipart(build_fake_post(None, None))
            .add_header("Accept", "application/json")
            .add_header("Cookie", format!("jwt={}", token0))
            .await
            .json::<Post>();

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": deliver_post.id.unwrap().to_raw() }))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_failure();
        assert!(delivered_response.text().contains("Forbidden"))
    }
);

test_with_server!(
    try_to_deliver_task_request_some_user,
    |server, ctx_state, config| {
        let (server, user, _, token) = create_fake_login_test_user(&server).await;
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
                "participant": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

        let disc = DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap());
        let deliver_post = server
            .post(format!("/api/discussions/{}/posts", disc.to_raw()).as_str())
            .multipart(build_fake_post(None, None))
            .add_header("Accept", "application/json")
            .add_header("Cookie", format!("jwt={}", token))
            .await
            .json::<Post>();

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": deliver_post.id.unwrap().to_raw() }))
            .add_header("Cookie", format!("jwt={}", token))
            .add_header("Accept", "application/json")
            .await;
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
            "participant": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();

    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();
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
             "participant": Some(user.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();

    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();
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
            "participant": Some(user.id.as_ref().unwrap().to_raw()),
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
            "participant": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

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
            "participant": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
            "acceptance_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

    let _ = state
        .db
        .client
        .query("UPDATE $id SET acceptance_period=0;")
        .bind(("id", get_str_thing(&task_id).unwrap()))
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
    let disc = DiscussionDbService::get_profile_discussion_id(user0.id.as_ref().unwrap());
    let deliver_post = create_fake_post(server, &disc, None, None).await;
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
            "participant": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

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
        .bind(("id", get_str_thing(&task_id).unwrap()))
        .await;

    let response = server
        .post(&format!("/api/tasks/{}/deliver", task_id))
        .json(&json!({"post_id":  deliver_post.id}))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_failure();
    assert!(response.text().contains("The delivery period has expired"));
});

test_with_server!(
    try_to_add_task_donor_without_balance,
    |server, state, config| {
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
                "participant": Some(user0.id.as_ref().unwrap().to_raw()),
                "delivery_period": 1,
            }))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

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
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

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
    let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

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
            "participant": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;

    task_request.assert_status_success();
    let task_id = task_request.json::<TaskRequest>().id.unwrap();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participant": Some(user0.id.as_ref().unwrap().to_raw()),
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
        .bind(("id", task_id))
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
    delivered_task_request_with_private_delivery_post,
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
                "participant": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

        let accept_response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();
        let disc = DiscussionDbService::get_profile_discussion_id(user0.id.as_ref().unwrap());

        let title = faker::lorem::en::Sentence(7..20).fake::<String>();
        let data = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content")
            .add_text("users", user1.id.as_ref().unwrap().to_raw());

        let deliver_post = server
            .post(format!("/api/discussions/{disc}/posts").as_str())
            .multipart(data)
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await
            .json::<Post>();

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": deliver_post.id.as_ref().unwrap().to_raw() }))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_success();

        let post_view = PostDbService {
            db: &ctx_state.db.client,
            ctx: &Ctx::new(Ok(user0.id.as_ref().unwrap().to_raw()), false),
        }
        .get_view_by_id::<PostAccessView>(deliver_post.id.as_ref().unwrap().to_raw().as_str(), None)
        .await
        .unwrap();

        assert_eq!(post_view.users.len(), 2);
        let post_creator_access = post_view
            .users
            .iter()
            .find(|u| u.user == *user0.id.as_ref().unwrap())
            .unwrap();

        assert_eq!(post_creator_access.role, Role::Member.to_string());

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
    try_to_deliver_task_request_with_private_delivery_post_without_donors_access,
    |server, ctx_state, config| {
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let (server, user2, _, _token2) = create_fake_login_test_user(&server).await;
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
                "participant": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

        let accept_response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();
        let disc = DiscussionDbService::get_profile_discussion_id(user0.id.as_ref().unwrap());

        let title = faker::lorem::en::Sentence(7..20).fake::<String>();
        let data = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content")
            .add_text("users", user2.id.as_ref().unwrap().to_raw());

        let deliver_post = server
            .post(format!("/api/discussions/{disc}/posts").as_str())
            .multipart(data)
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await
            .json::<Post>();

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": deliver_post.id.as_ref().unwrap().to_raw() }))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_failure();

        assert!(delivered_response
            .text()
            .contains("All donors must have view access to the delivery post"))
    }
);

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
                "participant": Some(user0.id.as_ref().unwrap().to_raw()),
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
    given_tasks_public_disc_private_post,
    |server, ctx_state, config| {
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let (server, user2, _, token2) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());

        let title = faker::lorem::en::Sentence(7..20).fake::<String>();
        let data = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content")
            .add_text("users", user2.id.as_ref().unwrap().to_raw())
            .add_text("users", user0.id.as_ref().unwrap().to_raw());

        let post = server
            .post(format!("/api/discussions/{}/posts", disc_id.to_raw()).as_str())
            .multipart(data)
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await
            .json::<Post>();

        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id.as_ref().unwrap().to_raw()).as_str())
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
            .post(&format!(
                "/api/posts/{}/remove_users",
                post.id.as_ref().unwrap().to_raw()
            ))
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
    }
);

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
                "participant": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<TaskRequest>().id.unwrap().to_raw();

        let accept_response = server
            .post(&format!("/api/tasks/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();
        let disc = DiscussionDbService::get_profile_discussion_id(user0.id.as_ref().unwrap());
        let deliver_post = server
            .post(format!("/api/discussions/{}/posts", disc.to_raw()).as_str())
            .multipart(build_fake_post(None, None))
            .add_header("Accept", "application/json")
            .add_header("Cookie", format!("jwt={}", token0))
            .await
            .json::<Post>();

        let delivered_response = server
            .post(&format!("/api/tasks/{}/deliver", task_id))
            .json(&json!({"post_id": deliver_post.id.as_ref().unwrap().to_raw() }))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_success();

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
        assert_eq!(posts[0].id, deliver_post.id)
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
    let task = task_request.json::<TaskRequest>();
    let task_id = task.id.as_ref().unwrap().to_raw();
    let get_task_response = server
        .get(&format!("/api/tasks/{}", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    get_task_response.assert_status_success();
    let task_view = get_task_response.json::<TaskRequestView>();
    assert_eq!(task_view.id, task.id.unwrap());

    let get_task_response = server
        .get(&format!("/api/tasks/{}", task_id))
        .add_header("Cookie", format!("jwt={}", token2))
        .add_header("Accept", "application/json")
        .await;
    get_task_response.assert_status_success();
    let task_view = get_task_response.json::<TaskRequestView>();
    assert_eq!(task_view.id, get_str_thing(&task_id).unwrap());

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
                "participant": user1.id.as_ref().unwrap().to_raw()
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
                "participant": user0.id.as_ref().unwrap().to_raw()
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
