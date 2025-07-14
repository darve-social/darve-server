mod helpers;

use darve_server::interfaces::repositories::verification_code::VerificationCodeRepositoryInterface;
use darve_server::{
    entities::{
        user_auth::{authentication_entity::AuthType, local_user_entity::LocalUserDbService},
        verification_code::VerificationCodeFor,
    },
    middleware::ctx::Ctx,
};
use fake::{faker, Fake};
use serde_json::json;

use crate::helpers::create_fake_login_test_user;

test_with_server!(test_forgot_password_success, |server, ctx_state, config| {
    let (_, user, _password, _) = create_fake_login_test_user(&server).await;

    let email = faker::internet::en::FreeEmail().fake::<String>();

    let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), false);

    let user_db = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };

    let _ = user_db
        .set_user_email(user.id.as_ref().unwrap().clone(), email.clone())
        .await;

    let response = server
        .post("/api/forgot_password")
        .json(&json!({
            "email_or_username": email
        }))
        .await;

    response.assert_status_success();

    let user_code = ctx_state
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
});

test_with_server!(
    test_forgot_password_by_username_success,
    |server, ctx_state, config| {
        let (_, user, _password, _) = create_fake_login_test_user(&server).await;

        let email = faker::internet::en::FreeEmail().fake::<String>();

        let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), false);

        let user_db = LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        };

        let _ = user_db
            .set_user_email(user.id.as_ref().unwrap().clone(), email.clone())
            .await;

        let response = server
            .post("/api/forgot_password")
            .json(&json!({
                "email_or_username": user.username.clone()
            }))
            .await;

        response.assert_status_success();

        let user_code = ctx_state
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
);

test_with_server!(
    test_forgot_password_not_exists_email,
    |server, ctx_state, config| {
        let (_, _, _password, _) = create_fake_login_test_user(&server).await;

        let email = faker::internet::en::FreeEmail().fake::<String>();

        let response = server
            .post("/api/forgot_password")
            .json(&json!({
                "email_or_username": email
            }))
            .await;

        response.assert_status_not_found();
    }
);

test_with_server!(
    test_forgot_password_invalid_email,
    |server, ctx_state, config| {
        let (_, _, _password, _) = create_fake_login_test_user(&server).await;

        let response = server
            .post("/api/forgot_password")
            .json(&json!({
                "email_or_username": "email"
            }))
            .await;

        response.assert_status_unprocessable_entity();
    }
);

test_with_server!(
    test_forgot_password_by_user_has_not_password_yet,
    |server, ctx_state, config| {
        let (_, user, _password, _) = create_fake_login_test_user(&server).await;

        let email = faker::internet::en::FreeEmail().fake::<String>();

        let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), false);

        let user_db = LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        };

        let _ = user_db
            .set_user_email(user.id.as_ref().unwrap().clone(), email.clone())
            .await;

        ctx_state
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
                "email_or_username": email
            }))
            .await;

        response.assert_status_failure();
        println!(">{}", response.text());
        assert!(response.text().contains("User has not set password yet"));
    }
);

test_with_server!(test_reset_password_success, |server, ctx_state, config| {
    let (_, user, password, _) = create_fake_login_test_user(&server).await;

    let email = faker::internet::en::FreeEmail().fake::<String>();

    let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), false);

    let user_db = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };

    let _ = user_db
        .set_user_email(user.id.as_ref().unwrap().clone(), email.clone())
        .await;

    let response = server
        .post("/api/forgot_password")
        .json(&json!({
            "email_or_username": email
        }))
        .await;

    response.assert_status_success();

    let user_code = ctx_state
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
            "email_or_username": email,
            "code": user_code.code,
            "password": "newPassword124"
        }))
        .await;

    response.assert_status_success();

    let user_code = ctx_state
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
});

test_with_server!(
    test_reset_password_by_username_success,
    |server, ctx_state, config| {
        let (_, user, password, _) = create_fake_login_test_user(&server).await;

        let email = faker::internet::en::FreeEmail().fake::<String>();

        let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), false);

        let user_db = LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        };

        let _ = user_db
            .set_user_email(user.id.as_ref().unwrap().clone(), email.clone())
            .await;

        let response = server
            .post("/api/forgot_password")
            .json(&json!({
                "email_or_username": user.username
            }))
            .await;

        response.assert_status_success();

        let user_code = ctx_state
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
                "email_or_username": email,
                "code": user_code.code,
                "password": "newPassword124"
            }))
            .await;

        response.assert_status_success();

        let user_code = ctx_state
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
);

test_with_server!(
    test_reset_password_to_many_requests,
    |server, ctx_state, config| {
        let (_, user, _, _) = create_fake_login_test_user(&server).await;

        let email = faker::internet::en::FreeEmail().fake::<String>();

        let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), false);

        let user_db = LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        };

        let _ = user_db
            .set_user_email(user.id.as_ref().unwrap().clone(), email.clone())
            .await;

        let response = server
            .post("/api/forgot_password")
            .json(&json!({
                "email_or_username": email
            }))
            .await;

        response.assert_status_success();

        let response = server
            .post("/api/reset_password")
            .json(&json!({
                "email_or_username": email,
                "code": "000000",
                "password": "newPassword124"
            }))
            .await;

        response.assert_status_failure();

        let response = server
            .post("/api/reset_password")
            .json(&json!({
                "email_or_username": email,
                "code": "000000",
                "password": "newPassword124"
            }))
            .await;

        response.assert_status_failure();

        let response = server
            .post("/api/reset_password")
            .json(&json!({
                "email_or_username": email,
                "code": "000000",
                "password": "newPassword124"
            }))
            .await;

        response.assert_status_failure();

        let response = server
            .post("/api/reset_password")
            .json(&json!({
                "email_or_username": email,
                "code": "000000",
                "password": "newPassword124"
            }))
            .await;

        response.assert_status_failure();
        assert!(response
            .text()
            .contains("Too many attempts. Wait and start new verification"));
    }
);

test_with_server!(
    test_reset_password_invalid_params,
    |server, ctx_state, config| {
        let _ = create_fake_login_test_user(&server).await;

        let response = server
            .post("/api/reset_password")
            .json(&json!({
                "email_or_username": "email",
                "code": "000000",
                "password": "newPassword124"
            }))
            .await;

        response.assert_status_unprocessable_entity();

        let response = server
            .post("/api/reset_password")
            .json(&json!({
                "email_or_username": "asdasd@asd.com",
                "code": "0000",
                "password": "newPassword124"
            }))
            .await;

        response.assert_status_unprocessable_entity();

        let response = server
            .post("/api/reset_password")
            .json(&json!({
                "email_or_username": "asdasd@asd.com",
                "code": "000000",
                "password": "124"
            }))
            .await;

        response.assert_status_unprocessable_entity();
    }
);
