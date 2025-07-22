mod helpers;

use crate::helpers::create_fake_login_test_user;
use chrono::Utc;
use darve_server::{
    entities::{
        community::{
            community_entity::CommunityDbService,
            discussion_entity::{Discussion, DiscussionDbService},
            post_entity::Post,
        },
        task::task_request_entity::TaskRequest,
        task_request_user::TaskParticipantStatus,
        wallet::wallet_entity::{CurrencySymbol, WalletDbService},
    },
    middleware::{ctx::Ctx, utils::string_utils::get_str_thing},
    routes::tasks::TaskRequestView,
};

use fake::{faker, Fake};
use helpers::post_helpers::{build_fake_post, create_fake_post};
use reqwest::StatusCode;
use serde_json::json;
use surrealdb::sql::Thing;

test_with_server!(created_closed_task_request, |server, ctx_state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = Thing::from((
        DiscussionDbService::get_table_name().as_ref(),
        user1.id.as_ref().unwrap().id.to_raw().as_ref(),
    ));
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user1.id.as_ref().unwrap().to_raw(),
            1000
        ))
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
    let tasks = task_request.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert!(first.due_at > Utc::now());
    assert!(first.participants.is_some());
    assert_eq!(first.participants.as_ref().unwrap().len(), 1);
    let task_user = first.participants.as_ref().unwrap().first().unwrap();
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
    assert_eq!(
        participator.user.as_ref().unwrap().id,
        user1.id.as_ref().unwrap().clone()
    );
    assert_eq!(participator.amount, 100);
});

test_with_server!(accepted_closed_task_request, |server, ctx_state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = Thing::from((
        DiscussionDbService::get_table_name().as_ref(),
        user1.id.as_ref().unwrap().id.to_raw().as_ref(),
    ));
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user1.id.as_ref().unwrap().to_raw(),
            1000
        ))
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
    let tasks = task_request.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert!(first.participants.is_some());
    assert_eq!(first.participants.as_ref().unwrap().len(), 1);
    let task_user = first.participants.as_ref().unwrap().first().unwrap();
    assert_eq!(task_user.user.id, user0.id.as_ref().unwrap().clone());
    assert_eq!(task_user.status, TaskParticipantStatus::Accepted);
});

test_with_server!(accepted_opened_task_request, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = Thing::from((
        DiscussionDbService::get_table_name().as_ref(),
        user1.id.as_ref().unwrap().id.to_raw().as_ref(),
    ));
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user1.id.as_ref().unwrap().to_raw(),
            1000
        ))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();
    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
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
    accept_response.assert_status_success();

    let task_request = server
        .get("/api/tasks/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let tasks = task_request.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert!(first.participants.is_some());
    assert_eq!(first.participants.as_ref().unwrap().len(), 2);
    let task_users = first.participants.as_ref().unwrap();
    assert_eq!(task_users.len(), 2);
    let task_user0 = task_users
        .iter()
        .find(|t| &t.user.id == user0.id.as_ref().unwrap())
        .unwrap();
    assert_eq!(task_user0.user.id, user0.id.as_ref().unwrap().clone());
    assert_eq!(task_user0.status, TaskParticipantStatus::Accepted);
    let task_user = task_users
        .iter()
        .find(|t| &t.user.id == user.id.as_ref().unwrap())
        .unwrap();
    assert_eq!(task_user.user.id, user.id.as_ref().unwrap().clone());
    assert_eq!(task_user.status, TaskParticipantStatus::Accepted);
});

test_with_server!(
    try_to_accept_task_request_after_rejected,
    |server, ctx_state, config| {
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user1.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user1.id.as_ref().unwrap().to_raw(),
                1000
            ))
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
        let disc_id = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user1.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user1.id.as_ref().unwrap().to_raw(),
                1000
            ))
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
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user0.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
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
        let (server, _user0, _, _token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user1.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user1.id.as_ref().unwrap().to_raw(),
                1000
            ))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();
        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id).as_str())
            .json(&json!({
                "offer_amount": Some(100),
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
        let disc_id = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user1.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user1.id.as_ref().unwrap().to_raw(),
                1000
            ))
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
    let disc_id = Thing::from((
        DiscussionDbService::get_table_name().as_ref(),
        user1.id.as_ref().unwrap().id.to_raw().as_ref(),
    ));
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user1.id.as_ref().unwrap().to_raw(),
            1000
        ))
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
        .get(&format!(
            "/test/api/endow/{}/{}",
            user1.id.as_ref().unwrap().to_raw(),
            1000
        ))
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
    let tasks = task_request.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert!(first.participants.is_some());
    assert_eq!(first.participants.as_ref().unwrap().len(), 1);
    let task_user = first.participants.as_ref().unwrap().first().unwrap();
    assert_eq!(task_user.user.id, user0.id.as_ref().unwrap().clone());
    assert_eq!(task_user.status, TaskParticipantStatus::Rejected);
});

test_with_server!(
    try_to_reject_task_request_after_delivered,
    |server, ctx_state, config| {
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user1.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user1.id.as_ref().unwrap().to_raw(),
                1000
            ))
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
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user0.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
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
        let disc_id = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user1.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user1.id.as_ref().unwrap().to_raw(),
                1000
            ))
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
    let disc_id = Thing::from((
        DiscussionDbService::get_table_name().as_ref(),
        user1.id.as_ref().unwrap().id.to_raw().as_ref(),
    ));
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user1.id.as_ref().unwrap().to_raw(),
            1000
        ))
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
    let disc = Thing::from((
        DiscussionDbService::get_table_name().as_ref(),
        user0.id.as_ref().unwrap().id.to_raw().as_ref(),
    ));
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
    let task_request = server
        .get("/api/tasks/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let tasks = task_request.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert!(first.participants.is_some());
    assert_eq!(first.participants.as_ref().unwrap().len(), 1);
    let task_user = first.participants.as_ref().unwrap().first().unwrap();
    assert_eq!(task_user.user.id, user0.id.as_ref().unwrap().clone());
    assert_eq!(task_user.status, TaskParticipantStatus::Delivered);
});

test_with_server!(
    try_to_deliver_task_request_after_rejected,
    |server, ctx_state, config| {
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let disc_id = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user1.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user1.id.as_ref().unwrap().to_raw(),
                1000
            ))
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
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user0.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
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
        let disc_id = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user1.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user1.id.as_ref().unwrap().to_raw(),
                1000
            ))
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

        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user0.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
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
        let disc_id = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user1.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let post = create_fake_post(server, &disc_id, None, None).await;

        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user1.id.as_ref().unwrap().to_raw(),
                1000
            ))
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

        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
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
    let (server, _user, _, token) = create_fake_login_test_user(&server).await;
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = Thing::from((
        DiscussionDbService::get_table_name().as_ref(),
        user1.id.as_ref().unwrap().id.to_raw().as_ref(),
    ));
    let post = create_fake_post(server, &disc_id, None, None).await;

    let post1 = create_fake_post(server, &disc_id, None, None).await;
    let post2 = create_fake_post(server, &disc_id, None, None).await;
    let post3 = create_fake_post(server, &disc_id, None, None).await;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user1.id.as_ref().unwrap().to_raw(),
            1000
        ))
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
        .post(format!("/api/posts/{}/tasks", post1.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post2.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
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
    let response = server
        .post(&format!("/api/tasks/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post3.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
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
    let tasks = response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let response = server
        .get("/api/tasks/received")
        .add_query_param("status", "Rejected")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 0);
    let response = server
        .get("/api/tasks/received?status=Accepted")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);

    let response = server
        .get("/api/tasks/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 3);
    let response = server
        .get("/api/tasks/received")
        .add_query_param("status", "Rejected")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let response = server
        .get("/api/tasks/received?status=Requested")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let response = server
        .get("/api/tasks/received?status=Accepted")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
});

test_with_server!(try_to_acceptance_task_expired, |server, state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = Thing::from((
        DiscussionDbService::get_table_name().as_ref(),
        user1.id.as_ref().unwrap().id.to_raw().as_ref(),
    ));
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user1.id.as_ref().unwrap().to_raw(),
            1000
        ))
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
    let disc = Thing::from((
        DiscussionDbService::get_table_name().as_ref(),
        user0.id.as_ref().unwrap().id.to_raw().as_ref(),
    ));
    let deliver_post = create_fake_post(server, &disc, None, None).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = Thing::from((
        DiscussionDbService::get_table_name().as_ref(),
        user1.id.as_ref().unwrap().id.to_raw().as_ref(),
    ));
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user1.id.as_ref().unwrap().to_raw(),
            1000
        ))
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
                "offer_amount": Some(100),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
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
            .post(
                format!(
                    "/api/discussions/{}/tasks",
                    created.id.as_ref().unwrap().to_raw()
                )
                .as_str(),
            )
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
            .get(&format!(
                "/test/api/endow/{}/{}",
                user1.id.as_ref().unwrap().to_raw(),
                10000
            ))
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
        .post(
            format!(
                "/api/discussions/{}/tasks",
                created.id.as_ref().unwrap().to_raw()
            )
            .as_str(),
        )
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
