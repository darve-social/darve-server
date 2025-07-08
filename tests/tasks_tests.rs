mod helpers;
use crate::helpers::create_fake_login_test_user;
use darve_server::{
    entities::{
        community::discussion_entity::DiscussionDbService, task_request_user::TaskRequestUserStatus,
    },
    middleware::utils::{request_utils::CreatedResponse, string_utils::get_str_thing},
    routes::task::task_request_routes::TaskRequestView,
};

use fake::{faker, Fake};
use helpers::post_helpers::{build_fake_post, create_fake_post};
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
        .post("/api/task_request")
        .json(&json!({
            "post_id": Some(post.id),
            "offer_amount": Some(1),
            "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_request = server
        .get("/api/task_request/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let tasks = task_request.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert!(first.to_users.is_some());
    assert_eq!(first.to_users.as_ref().unwrap().len(), 1);
    let task_user = first.to_users.as_ref().unwrap().first().unwrap();
    assert_eq!(task_user.user.id, user0.id.as_ref().unwrap().clone());
    assert_eq!(task_user.status, TaskRequestUserStatus::Requested);
    let task_request = server
        .get("/api/task_request/given")
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let tasks = task_request.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert_eq!(first.participants.len(), 1);
    let participator = first.participants.first().unwrap();
    assert_eq!(
        participator.user.as_ref().unwrap().id,
        user1.id.as_ref().unwrap().clone()
    );
    assert_eq!(participator.amount, 1)
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
        .post("/api/task_request")
        .json(&json!({
            "post_id": Some(post.id),
            "offer_amount": Some(1),
            "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<CreatedResponse>().id;
    let accept_response = server
        .post(&format!("/api/task_request/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_success();

    let task_request = server
        .get("/api/task_request/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let tasks = task_request.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert!(first.to_users.is_some());
    assert_eq!(first.to_users.as_ref().unwrap().len(), 1);
    let task_user = first.to_users.as_ref().unwrap().first().unwrap();
    assert_eq!(task_user.user.id, user0.id.as_ref().unwrap().clone());
    assert_eq!(task_user.status, TaskRequestUserStatus::Accepted);
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
        .post("/api/task_request")
        .json(&json!({
            "post_id": Some(post.id),
            "offer_amount": Some(1),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<CreatedResponse>().id;
    let accept_response = server
        .post(&format!("/api/task_request/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_failure();
    assert!(accept_response.text().contains("Forbidden"));

    let accept_response = server
        .post(&format!("/api/task_request/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_success();
    let accept_response = server
        .post(&format!("/api/task_request/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_success();

    let task_request = server
        .get("/api/task_request/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let tasks = task_request.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert!(first.to_users.is_some());
    assert_eq!(first.to_users.as_ref().unwrap().len(), 2);
    let task_users = first.to_users.as_ref().unwrap();
    assert_eq!(task_users.len(), 2);
    let task_user0 = task_users
        .iter()
        .find(|t| &t.user.id == user0.id.as_ref().unwrap())
        .unwrap();
    assert_eq!(task_user0.user.id, user0.id.as_ref().unwrap().clone());
    assert_eq!(task_user0.status, TaskRequestUserStatus::Accepted);
    let task_user = task_users
        .iter()
        .find(|t| &t.user.id == user.id.as_ref().unwrap())
        .unwrap();
    assert_eq!(task_user.user.id, user.id.as_ref().unwrap().clone());
    assert_eq!(task_user.status, TaskRequestUserStatus::Accepted);
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
            .post("/api/task_request")
            .json(&json!({
                "post_id": Some(post.id),
                "offer_amount": Some(1),
                "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<CreatedResponse>().id;
        let accept_response = server
            .post(&format!("/api/task_request/{}/reject", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();
        let accept_response = server
            .post(&format!("/api/task_request/{}/accept", task_id))
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
            .post("/api/task_request")
            .json(&json!({
                "post_id": Some(post.id),
                "offer_amount": Some(1),
                "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<CreatedResponse>().id;
        let accept_response = server
            .post(&format!("/api/task_request/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user0.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let deliver_post = server
            .post(format!("/api/discussion/{}/post", disc.to_raw()).as_str())
            .multipart(build_fake_post(None, None))
            .add_header("Accept", "application/json")
            .add_header("Cookie", format!("jwt={}", token0))
            .await
            .json::<CreatedResponse>();

        let mut multipart_data = axum_test::multipart::MultipartForm::new();
        multipart_data = multipart_data.add_text("post_id", deliver_post.id);

        let delivered_response = server
            .post(&format!("/api/task_request/{}/deliver", task_id))
            .multipart(multipart_data)
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_success();

        let accept_response = server
            .post(&format!("/api/task_request/{}/accept", task_id))
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
            .post("/api/task_request")
            .json(&json!({
                "post_id": Some(post.id),
                "offer_amount": Some(1),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<CreatedResponse>().id;
        let accept_response = server
            .post(&format!("/api/task_request/{}/accept", task_id))
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
            .post("/api/task_request")
            .json(&json!({
                "post_id": Some(post.id),
                "offer_amount": Some(1),
                "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<CreatedResponse>().id;
        let accept_response = server
            .post(&format!("/api/task_request/{}/accept", task_id))
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
        .post("/api/task_request")
        .json(&json!({
            "post_id": Some(post.id),
            "offer_amount": Some(1),
            "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<CreatedResponse>().id;
    let accept_response = server
        .post(&format!("/api/task_request/{}/reject", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_success();
});

test_with_server!(rejected_opened_task_request, |server, ctx_state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
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
        .post("/api/task_request")
        .json(&json!({
            "offer_amount": Some(1),
            "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<CreatedResponse>().id;
    let accept_response = server
        .post(&format!("/api/task_request/{}/reject", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_success();
    let task_request = server
        .get("/api/task_request/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let tasks = task_request.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert!(first.to_users.is_some());
    assert_eq!(first.to_users.as_ref().unwrap().len(), 1);
    let task_user = first.to_users.as_ref().unwrap().first().unwrap();
    assert_eq!(task_user.user.id, user0.id.as_ref().unwrap().clone());
    assert_eq!(task_user.status, TaskRequestUserStatus::Rejected);
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
            .post("/api/task_request")
            .json(&json!({
                "post_id": Some(post.id),
                "offer_amount": Some(1),
                "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<CreatedResponse>().id;
        let accept_response = server
            .post(&format!("/api/task_request/{}/accept", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        accept_response.assert_status_success();
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user0.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let deliver_post = server
            .post(format!("/api/discussion/{}/post", disc.to_raw()).as_str())
            .multipart(build_fake_post(None, None))
            .add_header("Accept", "application/json")
            .add_header("Cookie", format!("jwt={}", token0))
            .await
            .json::<CreatedResponse>();

        let mut multipart_data = axum_test::multipart::MultipartForm::new();
        multipart_data = multipart_data.add_text("post_id", deliver_post.id);

        let delivered_response = server
            .post(&format!("/api/task_request/{}/deliver", task_id))
            .multipart(multipart_data)
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        delivered_response.assert_status_success();

        let accept_response = server
            .post(&format!("/api/task_request/{}/reject", task_id))
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
            .post("/api/task_request")
            .json(&json!({
                "post_id": Some(post.id),
                "offer_amount": Some(1),
                "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<CreatedResponse>().id;
        let accept_response = server
            .post(&format!("/api/task_request/{}/reject", task_id))
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
        .post("/api/task_request")
        .json(&json!({
            "post_id": Some(post.id),
            "offer_amount": Some(1),
            "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<CreatedResponse>().id;
    let accept_response = server
        .post(&format!("/api/task_request/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    accept_response.assert_status_success();
    let disc = Thing::from((
        DiscussionDbService::get_table_name().as_ref(),
        user0.id.as_ref().unwrap().id.to_raw().as_ref(),
    ));
    let deliver_post = server
        .post(format!("/api/discussion/{}/post", disc.to_raw()).as_str())
        .multipart(build_fake_post(None, None))
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token0))
        .await
        .json::<CreatedResponse>();

    let mut multipart_data = axum_test::multipart::MultipartForm::new();
    multipart_data = multipart_data.add_text("post_id", deliver_post.id);

    let delivered_response = server
        .post(&format!("/api/task_request/{}/deliver", task_id))
        .multipart(multipart_data)
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    delivered_response.assert_status_success();
    let task_request = server
        .get("/api/task_request/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let tasks = task_request.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let first = tasks.first().unwrap();
    assert!(first.to_users.is_some());
    assert_eq!(first.to_users.as_ref().unwrap().len(), 1);
    let task_user = first.to_users.as_ref().unwrap().first().unwrap();
    assert_eq!(task_user.user.id, user0.id.as_ref().unwrap().clone());
    assert_eq!(task_user.status, TaskRequestUserStatus::Delivered);
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
            .post("/api/task_request")
            .json(&json!({
                "post_id": Some(post.id),
                "offer_amount": Some(1),
                "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<CreatedResponse>().id;
        let response = server
            .post(&format!("/api/task_request/{}/reject", task_id))
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user0.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let deliver_post = server
            .post(format!("/api/discussion/{}/post", disc.to_raw()).as_str())
            .multipart(build_fake_post(None, None))
            .add_header("Accept", "application/json")
            .add_header("Cookie", format!("jwt={}", token0))
            .await
            .json::<CreatedResponse>();

        let mut multipart_data = axum_test::multipart::MultipartForm::new();
        multipart_data = multipart_data.add_text("post_id", deliver_post.id);

        let delivered_response = server
            .post(&format!("/api/task_request/{}/deliver", task_id))
            .multipart(multipart_data)
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
            .post("/api/task_request")
            .json(&json!({
                "post_id": Some(post.id),
                "offer_amount": Some(1),
                "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<CreatedResponse>().id;
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user0.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let deliver_post = server
            .post(format!("/api/discussion/{}/post", disc.to_raw()).as_str())
            .multipart(build_fake_post(None, None))
            .add_header("Accept", "application/json")
            .add_header("Cookie", format!("jwt={}", token0))
            .await
            .json::<CreatedResponse>();

        let mut multipart_data = axum_test::multipart::MultipartForm::new();
        multipart_data = multipart_data.add_text("post_id", deliver_post.id);

        let delivered_response = server
            .post(&format!("/api/task_request/{}/deliver", task_id))
            .multipart(multipart_data)
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
            .post("/api/task_request")
            .json(&json!({
                "post_id": Some(post.id),
                "offer_amount": Some(1),
                "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let task_id = task_request.json::<CreatedResponse>().id;
        let disc = Thing::from((
            DiscussionDbService::get_table_name().as_ref(),
            user.id.as_ref().unwrap().id.to_raw().as_ref(),
        ));
        let deliver_post = server
            .post(format!("/api/discussion/{}/post", disc.to_raw()).as_str())
            .multipart(build_fake_post(None, None))
            .add_header("Accept", "application/json")
            .add_header("Cookie", format!("jwt={}", token))
            .await
            .json::<CreatedResponse>();

        let mut multipart_data = axum_test::multipart::MultipartForm::new();
        multipart_data = multipart_data.add_text("post_id", deliver_post.id);

        let delivered_response = server
            .post(&format!("/api/task_request/{}/deliver", task_id))
            .multipart(multipart_data)
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
        .post("/api/task_request")
        .json(&json!({
            "post_id": Some(post.id),
            "offer_amount": Some(1),
            "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();

    let task_request = server
        .post("/api/task_request")
        .json(&json!({
            "post_id": Some(post1.id),
            "offer_amount": Some(1),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();

    let task_request = server
        .post("/api/task_request")
        .json(&json!({
            "post_id": Some(post2.id),
            "offer_amount": Some(1),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();

    let task_id = task_request.json::<CreatedResponse>().id;
    let response = server
        .post(&format!("/api/task_request/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let response = server
        .post(&format!("/api/task_request/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    let task_request = server
        .post("/api/task_request")
        .json(&json!({
            "post_id": Some(post3.id),
            "offer_amount": Some(1),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<CreatedResponse>().id;

    let response = server
        .post(&format!("/api/task_request/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    let response = server
        .post(&format!("/api/task_request/{}/reject", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    let response = server
        .get("/api/task_request/received")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let response = server
        .get("/api/task_request/received")
        .add_query_param("status", "Rejected")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 0);
    let response = server
        .get("/api/task_request/received?status=Accepted")
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);

    let response = server
        .get("/api/task_request/received")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 3);
    let response = server
        .get("/api/task_request/received")
        .add_query_param("status", "Rejected")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let response = server
        .get("/api/task_request/received?status=Requested")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let tasks = response.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 1);
    let response = server
        .get("/api/task_request/received?status=Accepted")
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
        .post("/api/task_request")
        .json(&json!({
            "post_id": Some(post.id),
            "offer_amount": Some(1),
            "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "acceptance_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<CreatedResponse>().id;

    let _ = state
        .db
        .client
        .query("UPDATE $id SET acceptance_period=0;")
        .bind(("id", get_str_thing(&task_id).unwrap()))
        .await;

    let response = server
        .post(&format!("/api/task_request/{}/accept", task_id))
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
        .post("/api/task_request")
        .json(&json!({
            "post_id": Some(post.id),
            "offer_amount": Some(1),
            "to_user": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let task_id = task_request.json::<CreatedResponse>().id;

    let response = server
        .post(&format!("/api/task_request/{}/accept", task_id))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    let _ = state
        .db
        .client
        .query("UPDATE $id SET delivery_period=0;")
        .bind(("id", get_str_thing(&task_id).unwrap()))
        .await;
    let mut multipart_data = axum_test::multipart::MultipartForm::new();
    multipart_data = multipart_data.add_text("post_id", deliver_post.id);

    let response = server
        .post(&format!("/api/task_request/{}/deliver", task_id))
        .multipart(multipart_data)
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_failure();
    assert!(response.text().contains("The delivery period has expired"));
});
