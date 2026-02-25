use darve_server::{
    entities::{
        community::{community_entity::CommunityDbService, discussion_entity::DiscussionDbService},
        task_request::TaskRequestType,
        user_auth::local_user_entity::{LocalUserDbService, UserRole},
    },
    middleware::ctx::Ctx,
    models::view::task::TaskRequestView,
    services::discussion_service::CreateDiscussion,
};
use fake::{faker, Fake};
use serde_json::json;

use crate::helpers::create_fake_login_test_user;

mod helpers;

test_with_server!(
    try_to_create_public_task_to_public_discussion_by_user,
    |server, ctx_state, config| {
        let (_, user, _password, token) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap());

        let content = faker::lorem::en::Sentence(7..20).fake::<String>();
        server
            .post(format!("/api/discussions/{}/tasks", disc_id.to_raw()).as_str())
            .json(&json!({"content": content}))
            .add_header("Accept", "application/json")
            .add_header("Authorization", format!("Bearer {}", token))
            .await
            .assert_status_forbidden();
    }
);

test_with_server!(
    try_to_create_private_task_to_public_discussion_by_user,
    |server, ctx_state, config| {
        let (_, patricipant, _password, _) = create_fake_login_test_user(&server).await;
        let (_, user, _password, token) = create_fake_login_test_user(&server).await;
        let disc_id = DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap());

        let content = faker::lorem::en::Sentence(7..20).fake::<String>();
        server
            .post(format!("/api/discussions/{}/tasks", disc_id.to_raw()).as_str())
            .json(&json!({ "content": content, "participants":  vec![patricipant.id.as_ref().unwrap().to_raw()] }))
            .add_header("Accept", "application/json")
          .add_header("Authorization", format!("Bearer {}", token))
            .await
            .assert_status_forbidden();
    }
);

test_with_server!(
    try_to_create_a_task_to_public_admin_discussion_by_user,
    |server, ctx_state, config| {
        let user_repository = LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &Ctx::new(Ok("".to_string()), false),
        };
        let admins = user_repository.get_by_role(UserRole::Admin).await.unwrap();
        let admin = admins.first().unwrap();
        let disc_id = DiscussionDbService::get_profile_discussion_id(admin.id.as_ref().unwrap());
        let (_, _, _password, token) = create_fake_login_test_user(&server).await;
        let content = faker::lorem::en::Sentence(7..20).fake::<String>();
        server
            .post(format!("/api/discussions/{}/tasks", disc_id.to_raw()).as_str())
            .json(&json!({"content": content}))
            .add_header("Accept", "application/json")
            .add_header("Authorization", format!("Bearer {}", token))
            .await
            .assert_status_forbidden();
    }
);
test_with_server!(
    try_to_create_a_discussion_by_admin,
    |server, ctx_state, config| {
        let user_repository = LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &Ctx::new(Ok("".to_string()), false),
        };
        let admins = user_repository.get_by_role(UserRole::Admin).await.unwrap();
        let admin = admins.first().unwrap();
        let comm_id = CommunityDbService::get_profile_community_id(admin.id.as_ref().unwrap());

        let login_response = server
            .post("/api/login")
            .add_header("Accept", "application/json")
            .json(&serde_json::json!({
                "username_or_email": admin.username,
                "password": config.init_server_password
            }))
            .await;

        let json_response = login_response.json::<serde_json::Value>();
        let token = json_response["token"].as_str().unwrap();
        server
            .post("/api/discussions")
            .json(&CreateDiscussion {
                community_id: comm_id.to_raw(),
                title: "The Discussion".to_string(),
                image_uri: None,
                chat_user_ids: None,
                private_discussion_users_final: false,
            })
            .add_header("Accept", "application/json")
            .add_header("Authorization", format!("Bearer {}", token))
            .await
            .assert_status_forbidden();
    }
);

test_with_server!(
    create_public_task_to_public_discussion_by_admin,
    |server, ctx_state, config| {
        let user_repository = LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &Ctx::new(Ok("".to_string()), false),
        };
        let admins = user_repository.get_by_role(UserRole::Admin).await.unwrap();
        let admin = admins.first().unwrap();

        let login_response = server
            .post("/api/login")
            .add_header("Accept", "application/json")
            .json(&serde_json::json!({
                "username_or_email": admin.username,
                "password": config.init_server_password
            }))
            .await;

        let json_response = login_response.json::<serde_json::Value>();
        let token = json_response["token"].as_str().unwrap();
        let disc_id = DiscussionDbService::get_profile_discussion_id(admin.id.as_ref().unwrap());

        let content = faker::lorem::en::Sentence(7..20).fake::<String>();
        server
            .post(format!("/api/discussions/{}/tasks", disc_id.to_raw()).as_str())
            .json(&json!({"content": content}))
            .add_header("Accept", "application/json")
            .add_header("Authorization", format!("Bearer {}", token))
            .await
            .assert_status_success();
    }
);

test_with_server!(get_admins_tasks_by_user, |server, ctx_state, config| {
    let user_repository = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok("".to_string()), false),
    };
    let admins = user_repository.get_by_role(UserRole::Admin).await.unwrap();
    let admin = admins.first().unwrap();

    server
        .post("/api/login")
        .add_header("Accept", "application/json")
        .json(&serde_json::json!({
            "username_or_email": admin.username,
            "password": config.init_server_password
        }))
        .await
        .assert_status_success();

    server
        .get(&format!("/test/api/deposit/{}/{}", admin.username, 1000))
        .add_header("Accept", "application/json")
        .json("")
        .await
        .assert_status_success();

    let (_, _, _password, user_token) = create_fake_login_test_user(&server).await;
    let res = server
        .get("/api/admin/tasks")
        .add_header("Authorization", format!("Bearer {}", user_token))
        .add_header("Accept", "application/json")
        .await;
    res.assert_status_success();
    let tasks = res.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 4);

    let super_tasks = tasks
        .iter()
        .filter(|t| t.r#type == TaskRequestType::Public)
        .collect::<Vec<&TaskRequestView>>();

    assert_eq!(super_tasks.len(), 1);

    server
        .post(&format!(
            "/api/tasks/{}/accept",
            super_tasks.first().unwrap().id
        ))
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", user_token))
        .await
        .assert_status_success();
});

test_with_server!(get_admins_tasks_by_user_1, |server, ctx_state, config| {
    let user_repository = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok("".to_string()), false),
    };
    let admins = user_repository.get_by_role(UserRole::Admin).await.unwrap();
    let admin = admins.first().unwrap();

    server
        .post("/api/login")
        .add_header("Accept", "application/json")
        .json(&serde_json::json!({
            "username_or_email": admin.username,
            "password": config.init_server_password
        }))
        .await
        .assert_status_success();

    server
        .get(&format!("/test/api/deposit/{}/{}", admin.username, 1000))
        .add_header("Accept", "application/json")
        .json("")
        .await
        .assert_status_success();
    let (_, _user, _user_pwd, user_token) = create_fake_login_test_user(&server).await;
    let (_, _user1, _user1_pwd, user1_token) = create_fake_login_test_user(&server).await;
    let res = server
        .get("/api/admin/tasks")
        .add_header("Authorization", format!("Bearer {}", user_token))
        .add_header("Accept", "application/json")
        .await;
    res.assert_status_success();
    let tasks = res.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 4);

    let super_tasks = tasks
        .iter()
        .filter(|t| t.r#type == TaskRequestType::Public)
        .collect::<Vec<&TaskRequestView>>();
    let weekly_tasks = tasks
        .iter()
        .filter(|t| t.r#type == TaskRequestType::Private)
        .collect::<Vec<&TaskRequestView>>();

    assert_eq!(super_tasks.len(), 1);
    assert_eq!(weekly_tasks.len(), 3);
    let res = server
        .get("/api/admin/tasks")
        .add_header("Authorization", format!("Bearer {}", user1_token))
        .add_header("Accept", "application/json")
        .await;
    res.assert_status_success();
    let tasks = res.json::<Vec<TaskRequestView>>();
    assert_eq!(tasks.len(), 4);

    let super_tasks = tasks
        .iter()
        .filter(|t| t.r#type == TaskRequestType::Public)
        .collect::<Vec<&TaskRequestView>>();
    let weekly_tasks = tasks
        .iter()
        .filter(|t| t.r#type == TaskRequestType::Private)
        .collect::<Vec<&TaskRequestView>>();

    assert_eq!(super_tasks.len(), 1);
    assert_eq!(weekly_tasks.len(), 3);
});
