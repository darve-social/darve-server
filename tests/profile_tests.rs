mod helpers;

use axum_test::multipart::MultipartForm;
use darve_server::{
    entities::{
        user_auth::{
            authentication_entity::AuthType,
            local_user_entity::{LocalUser, LocalUserDbService},
        },
        verification_code::VerificationCodeFor,
    },
    interfaces::repositories::verification_code_ifce::VerificationCodeRepositoryInterface,
    middleware::{self, utils::db_utils::UsernameIdent},
    routes::{community::profile_routes::ProfileView, users::SearchInput},
};
use helpers::{
    create_fake_login_test_user, create_login_test_user,
    user_helpers::{self, get_user, update_current_user},
};
use middleware::ctx::Ctx;
use serde_json::json;

test_with_server!(search_users, |server, ctx_state, config| {
    let username1 = "its_user_one".to_string();
    let username2 = "its_user_two".to_string();
    let username3 = "herodus".to_string();
    let (server, _) = create_login_test_user(&server, username1.clone()).await;
    let (server, _) = create_login_test_user(&server, username2.clone()).await;
    let (server, _) = create_login_test_user(&server, username3.clone()).await;

    let request = server
        .patch("/api/users/current")
        .multipart(
            MultipartForm::new()
                .add_text("username", "username_new")
                .add_text("full_name", "Full Name Userset"),
        )
        .add_header("Accept", "application/json")
        .await;

    request.assert_status_success();

    let res = user_helpers::search_users(
        &server,
        &SearchInput {
            query: "rset".to_string(),
            start: None,
            count: None,
        },
    )
    .await;
    assert_eq!(res.len(), 1);

    let res = user_helpers::search_users(
        &server,
        &SearchInput {
            query: "user".to_string(),
            start: None,
            count: None,
        },
    )
    .await;
    assert_eq!(res.len(), 3);

    let res = user_helpers::search_users(
        &server,
        &SearchInput {
            query: "Userse".to_string(),
            start: None,
            count: None,
        },
    )
    .await;
    assert_eq!(res.len(), 1);

    let res = user_helpers::search_users(
        &server,
        &SearchInput {
            query: "one".to_string(),
            start: None,
            count: None,
        },
    )
    .await;
    assert_eq!(res.len(), 1);

    let res = user_helpers::search_users(
        &server,
        &SearchInput {
            query: "unknown".to_string(),
            start: None,
            count: None,
        },
    )
    .await;
    assert_eq!(res.len(), 0);

    let res = user_helpers::search_users(
        &server,
        &SearchInput {
            query: "its".to_string(),
            start: None,
            count: None,
        },
    )
    .await;
    assert_eq!(res.len(), 2);

    let res = user_helpers::search_users(
        &server,
        &SearchInput {
            query: "hero".to_string(),
            start: None,
            count: None,
        },
    )
    .await;
    assert_eq!(res.len(), 0);

    let (server, _) = create_login_test_user(&server, String::from("abcdtest")).await;
    let (server, _) = create_login_test_user(&server, String::from("abcdtest1")).await;
    let (server, _) = create_login_test_user(&server, String::from("abcdtest2")).await;
    let (server, _) = create_login_test_user(&server, String::from("abcdtest3")).await;
    let (server, _) = create_login_test_user(&server, String::from("abcdtest4")).await;
    let res = user_helpers::search_users(
        &server,
        &SearchInput {
            query: "tes".to_string(),
            start: None,
            count: None,
        },
    )
    .await;
    assert_eq!(res.len(), 5);
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
        let ctx = Ctx::new(Ok(user_id.to_raw()), false);
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
            .get_by_user(&user_id.id.to_raw(), VerificationCodeFor::EmailVerification)
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
            .get_by_user(&user_id.id.to_raw(), VerificationCodeFor::EmailVerification)
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
        let ctx = Ctx::new(Ok(user_id.to_raw()), false);
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
            .get_by_user(&user_id.id.to_raw(), VerificationCodeFor::EmailVerification)
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
    let user = get_response.json::<ProfileView>();
    assert!(user.image_uri.as_ref().unwrap().contains(".png"));
});

test_with_server!(set_user_password, |server, ctx_state, config| {
    let new_password = "setPassword123";

    // Create and login user
    let (server, user, _old_password, _) = create_fake_login_test_user(&server).await;

    let response = server
        .post("/api/users/current/set_password/start")
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_failure();

    assert!(response.text().contains("The user has not set email yet"));

    let _ = ctx_state
        .db
        .client
        .query("UPDATE $user SET email_verified=$email")
        .bind(("email", "test@test.com"))
        .bind(("user", user.id.as_ref().unwrap().clone()))
        .await;

    let response = server
        .post("/api/users/current/set_password/start")
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_failure();
    assert!(response.text().contains("User has already set a password"));

    ctx_state
        .db
        .client
        .query("DELETE FROM authentication WHERE local_user=$user AND auth_type=$auth_type")
        .bind(("user", user.id.as_ref().unwrap().clone()))
        .bind(("auth_type", AuthType::PASSWORD))
        .await
        .unwrap();

    let response = server
        .post("/api/users/current/set_password/start")
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    let user_code = ctx_state
        .db
        .verification_code
        .get_by_user(
            &user.id.as_ref().unwrap().id.to_raw(),
            VerificationCodeFor::SetPassword,
        )
        .await
        .unwrap();

    let response = server
        .post("/api/users/current/set_password/confirm")
        .json(&json!({
             "password": new_password,
             "code": user_code.code
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
            "username": user.username,
            "password": new_password
        }))
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_success();

    // Test validation: password too short should fail
    let response = server
        .post("/api/users/current/set_password/confirm")
        .json(&serde_json::json!({
            "password": "123485",
            "code": "222222"
        }))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_failure();
});

test_with_server!(update_user_password, |server, ctx_state, config| {
    let new_password = "newPassword456";

    // Create and login user
    let (server, user, password, _) = create_fake_login_test_user(&server).await;

    let response = server
        .post("/api/users/current/update_password/start")
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_failure();
    assert!(response.text().contains("The user has not set email yet"));

    let _ = ctx_state
        .db
        .client
        .query("UPDATE $user SET email_verified=$email")
        .bind(("email", "test@test.com"))
        .bind(("user", user.id.as_ref().unwrap().clone()))
        .await;

    let response = server
        .post("/api/users/current/update_password/start")
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let user_code = ctx_state
        .db
        .verification_code
        .get_by_user(
            &user.id.as_ref().unwrap().id.to_raw(),
            VerificationCodeFor::UpdatePassword,
        )
        .await
        .unwrap();

    // Try to update password with wrong old password
    let response = server
        .post("/api/users/current/update_password/confrim")
        .json(&serde_json::json!({
            "old_password": "wrongPassword",
            "new_password": new_password,
            "code": user_code
        }))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_failure();

    // Update password with correct old password
    let response = server
        .post("/api/users/current/update_password/confirm")
        .json(&serde_json::json!({
            "old_password": password,
            "new_password": new_password,
            "code": user_code.code
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
            "username": user.username,
            "password": password
        }))
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_failure();

    // Login with new password (should succeed)
    let login_response = server
        .post("/api/login")
        .json(&serde_json::json!({
            "username": user.username,
            "password": new_password
        }))
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_success();
});

test_with_server!(update_social_links, |server, ctx_state, config| {
    let username1 = "its_user_one".to_string();
    let (server, _) = create_login_test_user(&server, username1.clone()).await;

    let social_links = vec![
        "https://x.com/example".to_string(),
        "https://instagram.com/example".to_string(),
        "https://youtube.com/example".to_string(),
    ];

    let mut multipart_form = MultipartForm::new();
    for link in social_links.iter() {
        multipart_form = multipart_form.add_text("social_links", link.clone());
    }

    let request = server
        .patch("/api/users/current")
        .multipart(multipart_form)
        .add_header("Accept", "application/json")
        .await;
    request.assert_status_success();
    let user = request.json::<LocalUser>();
    assert!(user.social_links.is_some());
    let user_links = user.social_links.unwrap();
    assert!(user_links.len() == social_links.len());
    assert_eq!(user_links, social_links);
});

test_with_server!(
    try_to_update_social_links_with_unsupported_domain,
    |server, ctx_state, config| {
        let username1 = "its_user_one".to_string();
        let (server, _) = create_login_test_user(&server, username1.clone()).await;

        let social_links = vec![
            "https://asdasdas.com/example".to_string(),
            "https://instagram.com/example".to_string(),
            "https://youtube.com/example".to_string(),
        ];

        let mut multipart_form = MultipartForm::new();
        for link in social_links.iter() {
            multipart_form = multipart_form.add_text("social_links", link.clone());
        }

        let request = server
            .patch("/api/users/current")
            .multipart(multipart_form)
            .add_header("Accept", "application/json")
            .await;
        request.assert_status_failure();
        assert!(request
            .text()
            .contains("Social link must be from Twitter, Instagram, YouTube, or Facebook"))
    }
);

test_with_server!(
    try_to_update_social_links_with_couple_of_the_same_domain,
    |server, ctx_state, config| {
        let username1 = "its_user_one".to_string();
        let (server, _) = create_login_test_user(&server, username1.clone()).await;

        let social_links = vec![
            "https://instagram.com".to_string(),
            "https://instagram.com".to_string(),
        ];

        let mut multipart_form = MultipartForm::new();
        for link in social_links.iter() {
            multipart_form = multipart_form.add_text("social_links", link.clone());
        }

        let request = server
            .patch("/api/users/current")
            .multipart(multipart_form)
            .add_header("Accept", "application/json")
            .await;

        request.assert_status_failure();
        assert!(request
            .text()
            .contains("Social link must be from Twitter, Instagram, YouTube, or Facebook"))
    }
);
test_with_server!(set_empty_social_links, |server, ctx_state, config| {
    let username1 = "its_user_one".to_string();
    let (server, _) = create_login_test_user(&server, username1.clone()).await;

    let social_links = vec!["https://instagram.com".to_string()];

    let mut multipart_form = MultipartForm::new();
    for link in social_links.iter() {
        multipart_form = multipart_form.add_text("social_links", link.clone());
    }

    let request = server
        .patch("/api/users/current")
        .multipart(multipart_form)
        .add_header("Accept", "application/json")
        .await;

    request.assert_status_success();

    let request = server
        .patch("/api/users/current")
        .multipart(MultipartForm::new().add_text("social_links", ""))
        .add_header("Accept", "application/json")
        .await;

    request.assert_status_success();
    let user = request.json::<LocalUser>();
    assert_eq!(user.social_links.unwrap().len(), 0);
});

test_with_server!(replace_social_links, |server, ctx_state, config| {
    let username1 = "its_user_one".to_string();
    let (server, _) = create_login_test_user(&server, username1.clone()).await;

    let social_links = vec!["https://instagram.com".to_string()];

    let mut multipart_form = MultipartForm::new();
    for link in social_links.iter() {
        multipart_form = multipart_form.add_text("social_links", link.clone());
    }

    let request = server
        .patch("/api/users/current")
        .multipart(multipart_form)
        .add_header("Accept", "application/json")
        .await;

    request.assert_status_success();
    let new_link = "https://x.com/123";
    let request = server
        .patch("/api/users/current")
        .multipart(MultipartForm::new().add_text("social_links", new_link))
        .add_header("Accept", "application/json")
        .await;

    request.assert_status_success();
    let user = request.json::<LocalUser>();
    assert!(user.social_links.unwrap().contains(&new_link.to_string()));
});

test_with_server!(replace_image_url, |server, ctx_state, config| {
    let username1 = "its_user_one".to_string();
    let (server, _) = create_login_test_user(&server, username1.clone()).await;

    let create_response = update_current_user(&server).await;
    create_response.assert_status_success();

    let request = server
        .patch("/api/users/current")
        .multipart(MultipartForm::new().add_text("image_url", ""))
        .add_header("Accept", "application/json")
        .await;

    request.assert_status_success();
    let user = request.json::<LocalUser>();
    assert!(user.image_uri.is_none());
});

test_with_server!(
    set_none_fullname_and_birth_date,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;

        assert!(user.full_name.is_some());
        assert!(user.birth_date.is_some());
        let request = server
            .patch("/api/users/current")
            .multipart(
                MultipartForm::new()
                    .add_text("full_name", "")
                    .add_text("birth_date", ""),
            )
            .add_header("Accept", "application/json")
            .await;

        request.assert_status_success();
        let user = request.json::<LocalUser>();
        assert!(user.full_name.is_none());
        assert!(user.birth_date.is_none());
    }
);

test_with_server!(update_bio, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;

    assert!(user.bio.is_none());
    let request = server
        .patch("/api/users/current")
        .multipart(MultipartForm::new().add_text("bio", "test"))
        .add_header("Accept", "application/json")
        .await;

    request.assert_status_success();
    let user = request.json::<LocalUser>();
    assert_eq!(user.bio, Some("test".to_string()));
    let request = server
        .patch("/api/users/current")
        .multipart(MultipartForm::new().add_text("bio", ""))
        .add_header("Accept", "application/json")
        .await;

    request.assert_status_success();
    let user = request.json::<LocalUser>();

    assert!(user.bio.is_none())
});
