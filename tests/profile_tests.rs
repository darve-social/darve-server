mod helpers;
use axum_test::multipart::MultipartForm;
use darve_server::{
    entities::{
        user_auth::local_user_entity::LocalUserDbService, verification_code::VerificationCodeFor,
    },
    interfaces::repositories::verification_code::VerificationCodeRepositoryInterface,
    middleware::{self, utils::db_utils::UsernameIdent},
    routes::community::profile_routes::{self},
};
use helpers::{
    create_fake_login_test_user, create_login_test_user,
    user_helpers::{self, get_user, update_current_user},
};
use middleware::ctx::Ctx;
use profile_routes::SearchInput;
use serde_json::json;

test_with_server!(search_users, |server, ctx_state, config| {
    let username1 = "its_user_one".to_string();
    let username2 = "its_user_two".to_string();
    let username3 = "herodus".to_string();
    let (server, _) = create_login_test_user(&server, username1.clone()).await;
    let (server, _) = create_login_test_user(&server, username2.clone()).await;
    let (server, _) = create_login_test_user(&server, username3.clone()).await;

    let request = server
        .post("/api/accounts/edit")
        .multipart(
            MultipartForm::new()
                .add_text("username", "username_new")
                .add_text("full_name", "Full Name Userset")
                .add_text("email", "ome@email.com"),
        )
        .add_header("Accept", "application/json")
        .await;
    request.assert_status_success();

    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "rset".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 1);

    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "user".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 3);

    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "Userse".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 1);

    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "one".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 1);

    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "unknown".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 0);

    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "its".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 2);

    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "hero".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 0);

    let (server, _) = create_login_test_user(&server, String::from("abcdtest")).await;
    let (server, _) = create_login_test_user(&server, String::from("abcdtest1")).await;
    let (server, _) = create_login_test_user(&server, String::from("abcdtest2")).await;
    let (server, _) = create_login_test_user(&server, String::from("abcdtest3")).await;
    let (server, _) = create_login_test_user(&server, String::from("abcdtest4")).await;
    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "tes".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 5);
});

test_with_server!(
    email_verification_and_confirmation,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let username = user.username;
        let user_id = user.id.clone().unwrap();
        let new_email = "asdasdasd@asdasd.com";

        let response = server
            .post("/api/users/current/email/verification/start")
            .json(&json!({ "email": "asasdasdas@asd.com"}))
            .add_header("Accept", "application/json")
            .await;

        response.assert_status_success();

        let db = &ctx_state.db.client;
        let ctx = Ctx::new(Ok(user_id.to_raw()), uuid::Uuid::new_v4(), false);
        let user_db = LocalUserDbService { db, ctx: &ctx };

        let response = server
            .post("/api/users/current/email/verification/start")
            .json(&json!({ "email": new_email}))
            .add_header("Accept", "application/json")
            .await;

        response.assert_status_success();

        let code = ctx_state
            .db
            .verification_code
            .get_by_user(&user_id.to_raw(), VerificationCodeFor::EmailVerification)
            .await
            .unwrap()
            .code;

        let response = server
            .post("/api/users/current/email/verification/confirm")
            .json(&json!({"code": code.clone(), "email": new_email }))
            .add_header("Accept", "application/json")
            .await;

        response.assert_status_success();

        let user = user_db.get(UsernameIdent(username).into()).await.unwrap();
        assert_eq!(user.email_verified, Some(new_email.to_string()));

        let code = ctx_state
            .db
            .verification_code
            .get_by_user(&user_id.to_raw(), VerificationCodeFor::EmailVerification)
            .await;

        assert!(code.is_err());
    }
);
test_with_server!(
    try_verification_email_with_already_verified_email,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let username = user.username;
        let user_id = user.id.clone().unwrap();
        let new_email = "asdasdasd@asdasd.com";

        let response = server
            .post("/api/users/current/email/verification/start")
            .json(&json!({ "email": "asasdasdas@asd.com"}))
            .add_header("Accept", "application/json")
            .await;

        response.assert_status_success();

        let db = &ctx_state.db.client;
        let ctx = Ctx::new(Ok(user_id.to_raw()), uuid::Uuid::new_v4(), false);
        let user_db = LocalUserDbService { db, ctx: &ctx };

        let response = server
            .post("/api/users/current/email/verification/start")
            .json(&json!({ "email": new_email}))
            .add_header("Accept", "application/json")
            .await;

        response.assert_status_success();

        let code = ctx_state
            .db
            .verification_code
            .get_by_user(&user_id.to_raw(), VerificationCodeFor::EmailVerification)
            .await
            .unwrap()
            .code;

        let response = server
            .post("/api/users/current/email/verification/confirm")
            .json(&json!({"code": code.clone(), "email": new_email }))
            .add_header("Accept", "application/json")
            .await;

        response.assert_status_success();

        let user = user_db.get(UsernameIdent(username).into()).await.unwrap();
        assert_eq!(user.email_verified, Some(new_email.to_string()));

        let (server, _user1, _, token1) = create_fake_login_test_user(&server).await;
        let response = server
            .post("/api/users/current/email/verification/start")
            .json(&json!({ "email": new_email}))
            .add_header("Accept", "application/json")
            .add_header("Cookie", format!("jwt={}", token1))
            .await;

        response.assert_status_failure();
        assert!(response.text().contains("The email is already used"))
    }
);

test_with_server!(
    email_confirmation_with_invalid_code,
    |server, ctx_state, config| {
        let (server, _, _pwd, _) = create_fake_login_test_user(&server).await;
        let new_email = "asdasdasd@asdasd.com";

        server
            .post("/api/users/current/email/verification/start")
            .json(&json!({ "email": new_email }))
            .add_header("Accept", "application/json")
            .await
            .assert_status_success();

        let response = server
            .post("/api/users/current/email/verification/confirm")
            .json(&json!({"code": "123456", "email": new_email }))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_failure();
        response.text().contains("Start new verification");

        let response = server
            .post("/api/users/current/email/verification/confirm")
            .json(&json!({"code": "123456", "email": new_email }))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_failure();
        response.text().contains("Start new verification");

        let response = server
            .post("/api/users/current/email/verification/confirm")
            .json(&json!({"code": "123456", "email": new_email }))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_failure();
        response.text().contains("Start new verification");

        let response = server
            .post("/api/users/current/email/verification/confirm")
            .json(&json!({"code": "123456", "email": new_email }))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_failure();
        assert!(response
            .text()
            .contains("Too many attempts. Wait and start new verification"));
    }
);

test_with_server!(update_user_avatar, |server, ctx_state, config| {
    let (_, local_user, _pwd, _) = create_fake_login_test_user(&server).await;

    let create_response = update_current_user(&server).await;
    create_response.assert_status_success();

    let get_response = get_user(&server, local_user.id.unwrap().to_raw().as_str()).await;
    get_response.assert_status_success();

    let user = get_response.json::<profile_routes::ProfilePage>();
    assert!(user
        .profile_view
        .unwrap()
        .image_uri
        .as_ref()
        .unwrap()
        .contains("profile_image.jpg"));
});

test_with_server!(set_user_password, |server, ctx_state, config| {
    let new_password = "setPassword123";

    // Create and login user
    let (server, local_user, _old_password, _) = create_fake_login_test_user(&server).await;

    // Set password using POST endpoint (no old password required)
    let response = server
        .post("/api/users/current/password")
        .json(&serde_json::json!({
            "password": new_password
        }))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_failure();
    assert!(response.text().contains("User has already set a password"));

    let _ = ctx_state
        .db
        .client
        .query("DELETE authentication WHERE local_user=$user")
        .bind(("user", local_user.id.unwrap()))
        .await;

    let response = server
        .post("/api/users/current/password")
        .json(&serde_json::json!({
            "password": new_password
        }))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    // Logout
    server.get("/logout").await;

    // Login with new password (should succeed)
    let login_response = server
        .post("/api/login")
        .json(&serde_json::json!({
            "username": local_user.username,
            "password": new_password
        }))
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_success();

    // Test validation: password too short should fail
    let response = server
        .post("/api/users/current/password")
        .json(&serde_json::json!({
            "password": "12345" // Less than 6 characters
        }))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_failure();
});

test_with_server!(update_user_password, |server, ctx_state, config| {
    let new_password = "newPassword456";

    // Create and login user
    let (server, local_user, password, _) = create_fake_login_test_user(&server).await;

    // Try to update password with wrong old password
    let response = server
        .patch("/api/users/current/password")
        .json(&serde_json::json!({
            "old_password": "wrongPassword",
            "new_password": new_password
        }))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_failure();

    // Update password with correct old password
    let response = server
        .patch("/api/users/current/password")
        .json(&serde_json::json!({
            "old_password": password,
            "new_password": new_password
        }))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    // Logout
    server.get("/logout").await;

    // Try to login with old password (should fail)
    let login_response = server
        .post("/api/login")
        .json(&serde_json::json!({
            "username": local_user.username,
            "password": password
        }))
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_failure();

    // Login with new password (should succeed)
    let login_response = server
        .post("/api/login")
        .json(&serde_json::json!({
            "username": local_user.username,
            "password": new_password
        }))
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_success();
});
