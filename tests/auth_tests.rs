mod helpers;

use darve_server::{
    entities::{
        user_auth::{authentication_entity::AuthType, local_user_entity::LocalUserDbService},
        verification_code::VerificationCodeFor,
    },
    interfaces::repositories::verification_code::VerificationCodeRepositoryInterface,
    middleware::ctx::Ctx,
};
use fake::{faker, Fake};
use serde_json::json;
use uuid::Uuid;

use crate::helpers::{create_fake_login_test_user, create_test_server};

#[tokio::test]
async fn test_forgot_password_success() {
    let (server, state) = create_test_server().await;
    let (_, user, _password) = create_fake_login_test_user(&server).await;

    let email = faker::internet::en::FreeEmail().fake::<String>();

    let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);

    let user_db = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    let _ = user_db
        .set_user_email(user.id.as_ref().unwrap().clone(), email.clone())
        .await;

    let response = server
        .post("/api/forgot_password")
        .json(&json!({
            "email": email
        }))
        .await;

    response.assert_status_success();

    let user_code = state
        .db
        .verification_code
        .get_by_user(
            &user.id.as_ref().unwrap().to_raw(),
            VerificationCodeFor::ResetPassword,
        )
        .await
        .unwrap();

    assert_eq!(user_code.user, user.id.as_ref().unwrap().to_raw());
    assert_eq!(user_code.email, email);
    assert_eq!(user_code.use_for, VerificationCodeFor::ResetPassword)
}

#[tokio::test]
async fn test_forgot_password_not_exists_email() {
    let (server, _) = create_test_server().await;
    let (_, _, _password) = create_fake_login_test_user(&server).await;

    let email = faker::internet::en::FreeEmail().fake::<String>();

    let response = server
        .post("/api/forgot_password")
        .json(&json!({
            "email": email
        }))
        .await;

    response.assert_status_not_found();
}

#[tokio::test]
async fn test_forgot_password_invalid_email() {
    let (server, _) = create_test_server().await;
    let (_, _, _password) = create_fake_login_test_user(&server).await;

    let response = server
        .post("/api/forgot_password")
        .json(&json!({
            "email": "email"
        }))
        .await;

    response.assert_status_unprocessable_entity();
}

#[tokio::test]
async fn test_forgot_password_by_user_has_not_password_yet() {
    let (server, state) = create_test_server().await;
    let (_, user, _password) = create_fake_login_test_user(&server).await;

    let email = faker::internet::en::FreeEmail().fake::<String>();

    let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);

    let user_db = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    let _ = user_db
        .set_user_email(user.id.as_ref().unwrap().clone(), email.clone())
        .await;

    state
        .db
        .client
        .query("DELETE FROM authentication WHERE local_user=$user AND auth_type=$auth_type")
        .bind(("user", user.id.as_ref().unwrap().clone()))
        .bind(("auth_type", AuthType::PASSWORD))
        .await
        .unwrap();

    let response = server
        .post("/api/forgot_password")
        .json(&json!({
            "email": email
        }))
        .await;

    response.assert_status_failure();
    println!(">{}", response.text());
    assert!(response.text().contains("User has not set password yet"));
}

#[tokio::test]
async fn test_reset_password_success() {
    let (server, state) = create_test_server().await;
    let (_, user, password) = create_fake_login_test_user(&server).await;

    let email = faker::internet::en::FreeEmail().fake::<String>();

    let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);

    let user_db = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    let _ = user_db
        .set_user_email(user.id.as_ref().unwrap().clone(), email.clone())
        .await;

    let response = server
        .post("/api/forgot_password")
        .json(&json!({
            "email": email
        }))
        .await;

    response.assert_status_success();

    let user_code = state
        .db
        .verification_code
        .get_by_user(
            &user.id.as_ref().unwrap().to_raw(),
            VerificationCodeFor::ResetPassword,
        )
        .await
        .unwrap();

    let response = server
        .post("/api/reset_password")
        .json(&json!({
            "email": email,
            "code": user_code.code,
            "password": "newPassword124"
        }))
        .await;

    response.assert_status_success();

    let user_code = state
        .db
        .verification_code
        .get_by_user(
            &user.id.as_ref().unwrap().to_raw(),
            VerificationCodeFor::ResetPassword,
        )
        .await;

    assert!(user_code.is_err());

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
    let response = server
        .post("/api/login")
        .json(&serde_json::json!({
            "username": user.username,
            "password": "newPassword124"
        }))
        .add_header("Accept", "application/json")
        .await;

    response.assert_status_success();
}

#[tokio::test]
async fn test_reset_password_to_many_requests() {
    let (server, state) = create_test_server().await;
    let (_, user, _) = create_fake_login_test_user(&server).await;

    let email = faker::internet::en::FreeEmail().fake::<String>();

    let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);

    let user_db = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    let _ = user_db
        .set_user_email(user.id.as_ref().unwrap().clone(), email.clone())
        .await;

    let response = server
        .post("/api/forgot_password")
        .json(&json!({
            "email": email
        }))
        .await;

    response.assert_status_success();

    let response = server
        .post("/api/reset_password")
        .json(&json!({
            "email": email,
            "code": "000000",
            "password": "newPassword124"
        }))
        .await;

    response.assert_status_failure();

    let response = server
        .post("/api/reset_password")
        .json(&json!({
            "email": email,
            "code": "000000",
            "password": "newPassword124"
        }))
        .await;

    response.assert_status_failure();

    let response = server
        .post("/api/reset_password")
        .json(&json!({
            "email": email,
            "code": "000000",
            "password": "newPassword124"
        }))
        .await;

    response.assert_status_failure();

    let response = server
        .post("/api/reset_password")
        .json(&json!({
            "email": email,
            "code": "000000",
            "password": "newPassword124"
        }))
        .await;

    response.assert_status_failure();
    assert!(response
        .text()
        .contains("Too many attempts. Wait and start new verification"));
}

#[tokio::test]
async fn test_reset_password_invalid_params() {
    let (server, _) = create_test_server().await;
    let (_, _, _) = create_fake_login_test_user(&server).await;

    let response = server
        .post("/api/reset_password")
        .json(&json!({
            "email": "email",
            "code": "000000",
            "password": "newPassword124"
        }))
        .await;

    response.assert_status_unprocessable_entity();

    let response = server
        .post("/api/reset_password")
        .json(&json!({
            "email": "asdasd@asd.com",
            "code": "0000",
            "password": "newPassword124"
        }))
        .await;

    response.assert_status_unprocessable_entity();

    let response = server
        .post("/api/reset_password")
        .json(&json!({
            "email": "asdasd@asd.com",
            "code": "000000",
            "password": "124"
        }))
        .await;

    response.assert_status_unprocessable_entity();
}
