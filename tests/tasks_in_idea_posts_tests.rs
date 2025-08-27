mod helpers;

use crate::helpers::create_fake_login_test_user;
use crate::helpers::post_helpers::create_post;
use axum_test::multipart::MultipartForm;
use darve_server::entities::community::discussion_entity::DiscussionDbService;
use darve_server::entities::community::post_entity::Post;
use darve_server::entities::community::post_entity::PostType;
use darve_server::entities::task::task_request_entity::TaskRequest;
use darve_server::entities::wallet::wallet_entity::CurrencySymbol;
use darve_server::models::view::task::TaskRequestView;
use fake::faker;
use fake::Fake;
use serde_json::json;

test_with_server!(
    forbidden_to_create_task_with_participant,
    |server, ctx_state, config| {
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

        let default_discussion =
            DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());

        let data = MultipartForm::new()
            .add_text("title", faker::name::en::Name().fake::<String>())
            .add_text("is_idea", true)
            .add_text(
                "content",
                faker::lorem::en::Sentence(7..20).fake::<String>(),
            );

        let res = create_post(server, &default_discussion, data).await;
        let post = res.json::<Post>();

        assert_eq!(post.r#type, PostType::Idea);

        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id.as_ref().unwrap().to_raw()).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                "participant": Some(user1.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
                "delivery_period": 1,
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;

        task_request.assert_status_forbidden();
    }
);

test_with_server!(
    forbidden_to_create_task_with_amount,
    |server, ctx_state, config| {
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

        let default_discussion =
            DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());

        let data = MultipartForm::new()
            .add_text("title", faker::name::en::Name().fake::<String>())
            .add_text("is_idea", true)
            .add_text(
                "content",
                faker::lorem::en::Sentence(7..20).fake::<String>(),
            );

        let res = create_post(server, &default_discussion, data).await;
        let post = res.json::<Post>();

        assert_eq!(post.r#type, PostType::Idea);

        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id.as_ref().unwrap().to_raw()).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
                "delivery_period": 1,
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;

        task_request.assert_status_forbidden();
    }
);

test_with_server!(create_task, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;

    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user.id.as_ref().unwrap().to_raw(),
            1000
        ))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

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

    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());

    let data = MultipartForm::new()
        .add_text("title", faker::name::en::Name().fake::<String>())
        .add_text("is_idea", true)
        .add_text(
            "content",
            faker::lorem::en::Sentence(7..20).fake::<String>(),
        );

    let res = create_post(server, &default_discussion, data).await;
    let post = res.json::<Post>();

    assert_eq!(post.r#type, PostType::Idea);

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id.as_ref().unwrap().to_raw()).as_str())
        .json(&json!({
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;

    task_request.assert_status_success();
});

test_with_server!(try_create_task_not_owner, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;

    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user.id.as_ref().unwrap().to_raw(),
            1000
        ))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

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

    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());

    let data = MultipartForm::new()
        .add_text("title", faker::name::en::Name().fake::<String>())
        .add_text("is_idea", true)
        .add_text(
            "content",
            faker::lorem::en::Sentence(7..20).fake::<String>(),
        );

    let res = create_post(server, &default_discussion, data).await;
    let post = res.json::<Post>();

    assert_eq!(post.r#type, PostType::Idea);

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id.as_ref().unwrap().to_raw()).as_str())
        .json(&json!({
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;

    task_request.assert_status_forbidden();
});

test_with_server!(
    forbidden_donate_by_owner_create_task,
    |server, ctx_state, config| {
        let (server, user, _, token) = create_fake_login_test_user(&server).await;

        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user.id.as_ref().unwrap().to_raw(),
                1000
            ))
            .add_header("Cookie", format!("jwt={}", token))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();

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

        let default_discussion =
            DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());

        let data = MultipartForm::new()
            .add_text("title", faker::name::en::Name().fake::<String>())
            .add_text("is_idea", true)
            .add_text(
                "content",
                faker::lorem::en::Sentence(7..20).fake::<String>(),
            );

        let res = create_post(server, &default_discussion, data).await;
        let post = res.json::<Post>();

        assert_eq!(post.r#type, PostType::Idea);

        let task_request = server
            .post(format!("/api/posts/{}/tasks", post.id.as_ref().unwrap().to_raw()).as_str())
            .json(&json!({
                "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
                "delivery_period": 1,
            }))
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await;

        task_request.assert_status_success();
        let task = task_request.json::<TaskRequest>();

        let task_request = server
            .post(&format!("/api/tasks/{}/donor", task.id.as_ref().unwrap()))
            .json(&json!({
                "amount": 100,
                "currency": CurrencySymbol::USD.to_string(),
            }))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_forbidden();
    }
);

test_with_server!(donate_by_guest_create_task, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;

    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user.id.as_ref().unwrap().to_raw(),
            1000
        ))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

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

    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user1.id.as_ref().unwrap());

    let data = MultipartForm::new()
        .add_text("title", faker::name::en::Name().fake::<String>())
        .add_text("is_idea", true)
        .add_text(
            "content",
            faker::lorem::en::Sentence(7..20).fake::<String>(),
        );

    let res = create_post(server, &default_discussion, data).await;
    let post = res.json::<Post>();

    assert_eq!(post.r#type, PostType::Idea);

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id.as_ref().unwrap().to_raw()).as_str())
        .json(&json!({
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;

    task_request.assert_status_success();
    let task = task_request.json::<TaskRequest>();

    let task_request = server
        .post(&format!("/api/tasks/{}/donor", task.id.as_ref().unwrap()))
        .json(&json!({
            "amount": 100,
            "currency": CurrencySymbol::USD.to_string(),
        }))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
});

test_with_server!(get_tasks_of_post, |server, ctx_state, config| {
    let (server, _, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user, _, token) = create_fake_login_test_user(&server).await;

    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user.id.as_ref().unwrap().to_raw(),
            1000
        ))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

    let data = MultipartForm::new()
        .add_text("title", faker::name::en::Name().fake::<String>())
        .add_text("is_idea", true)
        .add_text(
            "content",
            faker::lorem::en::Sentence(7..20).fake::<String>(),
        );

    let res = create_post(server, &default_discussion, data).await;
    let post = res.json::<Post>();

    assert_eq!(post.r#type, PostType::Idea);

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id.as_ref().unwrap().to_raw()).as_str())
        .json(&json!({
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;

    task_request.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id.as_ref().unwrap().to_raw()).as_str())
        .json(&json!({
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;

    task_request.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id.as_ref().unwrap().to_raw()).as_str())
        .json(&json!({
            "content":faker::lorem::en::Sentence(7..20).fake::<String>(),
            "delivery_period": 1,
        }))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;

    task_request.assert_status_success();

    let tasks_request = server
        .get(&format!("/api/posts/{}/tasks", post.id.as_ref().unwrap()))
        .json(&json!({
            "amount": 100,
            "currency": CurrencySymbol::USD.to_string(),
        }))
        .add_header("Cookie", format!("jwt={}", token))
        .add_header("Accept", "application/json")
        .await;
    tasks_request.assert_status_success();

    let tasks = tasks_request.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 3);

    let tasks_request = server
        .get(&format!("/api/posts/{}/tasks", post.id.as_ref().unwrap()))
        .json(&json!({
            "amount": 100,
            "currency": CurrencySymbol::USD.to_string(),
        }))
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await;
    tasks_request.assert_status_success();

    let tasks = tasks_request.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 3);
});
