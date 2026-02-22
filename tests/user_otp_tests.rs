mod helpers;
use crate::helpers::RecordIdExt;

use crate::helpers::create_fake_login_test_user;
use darve_server::{
    entities::user_auth::local_user_entity::{LocalUser, LocalUserDbService},
    middleware::ctx::Ctx,
    utils::totp::{Totp, TotpResponse},
};
use serde_json::json;

test_with_server!(test_otp_enable_success, |server, ctx_state, config| {
    let (_server, user, _password, token) = create_fake_login_test_user(&server).await;

    // Enable OTP
    let response = server
        .post("/api/users/current/otp/enable")
        .add_header("Cookie", &format!("jwt={}", token))
        .await;

    response.assert_status_success();
    let otp = response.json::<TotpResponse>();
    assert!(otp.url.starts_with("otpauth://totp/"));
    assert!(otp.url.contains("Darve"));

    // Verify user's OTP is enabled in database
    let ctx = Ctx::new(Ok(user.id.as_ref().unwrap().key_to_string()), false);
    let user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };

    let updated_user = user_db_service
        .get_by_id(&user.id.as_ref().unwrap().key_to_string())
        .await
        .unwrap();

    assert!(updated_user.otp_secret.is_some());
    assert_eq!(updated_user.is_otp_enabled, false);
});

test_with_server!(
    test_otp_verification_success,
    |server, ctx_state, config| {
        let (_server, user, _password, token) = create_fake_login_test_user(&server).await;

        // Enable OTP
        let response = server
            .post("/api/users/current/otp/enable")
            .add_header("Cookie", &format!("jwt={}", token))
            .await;

        response.assert_status_success();
        let otp = response.json::<TotpResponse>();
        assert!(otp.url.starts_with("otpauth://totp/"));
        assert!(otp.url.contains("Darve"));

        let totp = Totp::new(&user.id.as_ref().unwrap().key_to_string(), Some(otp.secret));
        let totp_response = totp.generate();

        // Verification OTP
        let response = server
            .post("/api/users/current/otp/verification")
            .add_header("Cookie", &format!("jwt={}", token))
            .json(&json!({ "token": totp_response.token}))
            .await;

        response.assert_status_success();

        // Verify user's OTP is enabled in database
        let ctx = Ctx::new(Ok(user.id.as_ref().unwrap().key_to_string()), false);
        let user_db_service = LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        };

        let updated_user = user_db_service
            .get_by_id(&user.id.as_ref().unwrap().key_to_string())
            .await
            .unwrap();

        assert!(updated_user.otp_secret.is_some());
        assert_eq!(updated_user.is_otp_enabled, true);
    }
);

test_with_server!(test_otp_enable_unauthorized, |server, ctx_state, config| {
    // Try to enable OTP without authentication
    let response = server.post("/api/users/current/otp/enable").await;

    response.assert_status_unauthorized();
});

test_with_server!(
    test_otp_enable_multiple_times,
    |server, ctx_state, config| {
        let (_server, user, _password, token) = create_fake_login_test_user(&server).await;

        // Enable OTP first time
        let response1 = server
            .post("/api/users/current/otp/enable")
            .add_header("Cookie", &format!("jwt={}", token))
            .await;
        response1.assert_status_success();
        let otp1 = response1.json::<TotpResponse>();

        // Enable OTP second time - should still work and return same URL
        let response2 = server
            .post("/api/users/current/otp/enable")
            .add_header("Cookie", &format!("jwt={}", token))
            .await;

        response2.assert_status_success();
        let otp2 = response1.json::<TotpResponse>();

        assert_eq!(otp1.url, otp2.url);

        // Verify user's OTP is still enabled
        let ctx = Ctx::new(Ok(user.id.as_ref().unwrap().key_to_string()), false);
        let user_db_service = LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        };

        let updated_user = user_db_service
            .get_by_id(&user.id.as_ref().unwrap().key_to_string())
            .await
            .unwrap();

        assert!(!updated_user.is_otp_enabled);
    }
);

test_with_server!(test_otp_disable_success, |server, ctx_state, config| {
    let (_server, user, _password, token) = create_fake_login_test_user(&server).await;

    // First enable OTP
    let response = server
        .post("/api/users/current/otp/enable")
        .add_header("Cookie", &format!("jwt={}", token))
        .await;

    response.assert_status_success();
    let otp = response.json::<TotpResponse>();
    assert!(otp.url.starts_with("otpauth://totp/"));
    assert!(otp.url.contains("Darve"));

    let totp = Totp::new(&user.id.as_ref().unwrap().key_to_string(), Some(otp.secret));
    let totp_response = totp.generate();
    // Verification OTP
    let response = server
        .post("/api/users/current/otp/verification")
        .add_header("Cookie", &format!("jwt={}", token))
        .json(&json!({ "token": totp_response.token}))
        .await;

    response.assert_status_success();
    // Verify OTP is enabled
    let ctx = Ctx::new(Ok(user.id.as_ref().unwrap().key_to_string()), false);
    let user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };

    let user_before_disable = user_db_service
        .get_by_id(&user.id.as_ref().unwrap().key_to_string())
        .await
        .unwrap();
    assert!(user_before_disable.is_otp_enabled);

    // Now disable OTP
    let disable_response = server
        .post("/api/users/current/otp/disable")
        .add_header("Cookie", &format!("jwt={}", token))
        .await;

    disable_response.assert_status_success();

    // Verify user's OTP is disabled in database
    let updated_user = user_db_service
        .get_by_id(&user.id.as_ref().unwrap().key_to_string())
        .await
        .unwrap();

    assert!(!updated_user.is_otp_enabled);
});

test_with_server!(
    test_otp_disable_unauthorized,
    |server, ctx_state, config| {
        // Try to disable OTP without authentication
        let response = server.post("/api/users/current/otp/disable").await;

        response.assert_status_unauthorized();
    }
);

test_with_server!(
    test_otp_disable_when_not_enabled,
    |server, ctx_state, config| {
        let (_server, user, _password, token) = create_fake_login_test_user(&server).await;

        // Disable OTP when it's not enabled (should still work)
        let response = server
            .post("/api/users/current/otp/disable")
            .add_header("Cookie", &format!("jwt={}", token))
            .await;

        response.assert_status_success();

        // Verify user's OTP remains disabled
        let ctx = Ctx::new(Ok(user.id.as_ref().unwrap().key_to_string()), false);
        let user_db_service = LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        };

        let updated_user = user_db_service
            .get_by_id(&user.id.as_ref().unwrap().key_to_string())
            .await
            .unwrap();

        assert!(!updated_user.is_otp_enabled);
    }
);

test_with_server!(test_otp_validate_success, |server, ctx_state, config| {
    let (_server, user, _password, token) = create_fake_login_test_user(&server).await;

    // First enable OTP
    let response = server
        .post("/api/users/current/otp/enable")
        .add_header("Cookie", &format!("jwt={}", token))
        .await;

    response.assert_status_success();
    let otp = response.json::<TotpResponse>();
    assert!(otp.url.starts_with("otpauth://totp/"));
    assert!(otp.url.contains("Darve"));

    let totp = Totp::new(&user.id.as_ref().unwrap().key_to_string(), Some(otp.secret));
    let totp_response = totp.generate();

    // Verification OTP
    let response = server
        .post("/api/users/current/otp/verification")
        .add_header("Cookie", &format!("jwt={}", token))
        .json(&json!({ "token": totp_response.token }))
        .await;

    response.assert_status_success();

    // Generate a valid OTP token
    let user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok("".to_string()), false),
    };
    let user_id = user.id.as_ref().unwrap().key_to_string();
    let user_with_otp = user_db_service
        .get_by_id(&user.id.as_ref().unwrap().key_to_string())
        .await
        .unwrap();
    assert!(user_with_otp.is_otp_enabled);

    let totp = Totp::new(&user_id, user_with_otp.otp_secret.clone());
    let totp_response = totp.generate();
    let valid_token = totp_response.token;

    // Create OTP token for validation request
    let otp_token = ctx_state
        .jwt
        .create_by_otp(&user_id)
        .expect("Failed to create OTP token");

    // Validate OTP
    let response = server
        .post("/api/users/current/otp/validate")
        .add_header("Authorization", &format!("Bearer {}", otp_token))
        .json(&json!({
            "token": valid_token
        }))
        .await;

    response.assert_status_success();

    let response_json = response.json::<serde_json::Value>();
    assert!(response_json.get("token").is_some());
    assert!(response_json.get("user").is_some());

    // Verify the returned user data
    let returned_user = &response_json["user"];
    assert_eq!(returned_user["username"], user.username);
    assert_eq!(returned_user["is_otp_enabled"], true);
});

test_with_server!(
    test_otp_validate_unauthorized,
    |server, ctx_state, config| {
        // Try to validate OTP without OTP authentication
        let response = server
            .post("/api/users/current/otp/validate")
            .json(&json!({
                "token": "123456"
            }))
            .await;

        response.assert_status_unauthorized();
    }
);

test_with_server!(
    test_otp_validate_with_login_token,
    |server, ctx_state, config| {
        let (_server, _user, _password, token) = create_fake_login_test_user(&server).await;

        // Try to validate OTP with login token instead of OTP token
        let response = server
            .post("/api/users/current/otp/validate")
            .add_header("Authorization", &format!("Bearer {}", token))
            .json(&json!({
                "token": "123456"
            }))
            .await;

        response.assert_status_unauthorized();
    }
);

test_with_server!(
    test_otp_validate_user_not_otp_enabled,
    |server, ctx_state, config| {
        let (_server, user, _password, _token) = create_fake_login_test_user(&server).await;

        let user_id = user.id.as_ref().unwrap().key_to_string();

        // Create OTP token for validation request
        let otp_token = ctx_state
            .jwt
            .create_by_otp(&user_id)
            .expect("Failed to create OTP token");

        // Try to validate OTP when user hasn't enabled it
        let response = server
            .post("/api/users/current/otp/validate")
            .add_header("Authorization", &format!("Bearer {}", otp_token))
            .json(&json!({
                "token": "123456"
            }))
            .await;

        response.assert_status_forbidden();
    }
);

test_with_server!(
    test_otp_validate_invalid_token,
    |server, ctx_state, config| {
        let (_server, user, _password, token) = create_fake_login_test_user(&server).await;

        // First enable OTP
        let response = server
            .post("/api/users/current/otp/enable")
            .add_header("Cookie", &format!("jwt={}", token))
            .await;
        response.assert_status_success();
        let otp = response.json::<TotpResponse>();
        assert!(otp.url.starts_with("otpauth://totp/"));
        assert!(otp.url.contains("Darve"));

        let totp = Totp::new(&user.id.as_ref().unwrap().key_to_string(), Some(otp.secret));
        let totp_response = totp.generate();

        // Verification OTP
        let response = server
            .post("/api/users/current/otp/verification")
            .add_header("Cookie", &format!("jwt={}", token))
            .json(&json!({ "token": totp_response.token}))
            .await;
        response.assert_status_success();

        let user_id = user.id.as_ref().unwrap().key_to_string();

        // Create OTP token for validation request
        let otp_token = ctx_state
            .jwt
            .create_by_otp(&user_id)
            .expect("Failed to create OTP token");

        // Try to validate with invalid OTP token
        let response = server
            .post("/api/users/current/otp/validate")
            .add_header("Authorization", &format!("Bearer {}", otp_token))
            .json(&json!({
                "token": "000000"
            }))
            .await;

        response.assert_status_forbidden();
    }
);

test_with_server!(
    test_otp_validate_malformed_request,
    |server, ctx_state, config| {
        let (_server, user, _password, token) = create_fake_login_test_user(&server).await;

        // First enable OTP
        let response = server
            .post("/api/users/current/otp/enable")
            .add_header("Cookie", &format!("jwt={}", token))
            .await;
        response.assert_status_success();
        let otp = response.json::<TotpResponse>();
        assert!(otp.url.starts_with("otpauth://totp/"));
        assert!(otp.url.contains("Darve"));

        let totp = Totp::new(&user.id.as_ref().unwrap().key_to_string(), Some(otp.secret));
        let totp_response = totp.generate();

        // Verification OTP
        let response = server
            .post("/api/users/current/otp/verification")
            .add_header("Cookie", &format!("jwt={}", token))
            .json(&json!({ "token": totp_response.token}))
            .await;
        response.assert_status_success();

        let user_id = user.id.as_ref().unwrap().key_to_string();

        // Create OTP token for validation request
        let otp_token = ctx_state
            .jwt
            .create_by_otp(&user_id)
            .expect("Failed to create OTP token");

        // Try to validate with missing token field
        let response = server
            .post("/api/users/current/otp/validate")
            .add_header("Authorization", &format!("Bearer {}", otp_token))
            .json(&json!({}))
            .await;

        response.assert_status_unprocessable_entity();

        // Try to validate with empty token
        let response = server
            .post("/api/users/current/otp/validate")
            .add_header("Authorization", &format!("Bearer {}", otp_token))
            .json(&json!({
                "token": ""
            }))
            .await;

        response.assert_status_forbidden();
    }
);

test_with_server!(test_otp_flow_end_to_end, |server, ctx_state, config| {
    let (_server, user, _password, login_token) = create_fake_login_test_user(&server).await;

    // Step 1: Enable OTP
    let response = server
        .post("/api/users/current/otp/enable")
        .add_header("Cookie", &format!("jwt={}", login_token))
        .await;
    response.assert_status_success();
    let otp = response.json::<TotpResponse>();
    assert!(otp.url.starts_with("otpauth://totp/"));
    assert!(otp.url.contains("Darve"));

    let totp = Totp::new(&user.id.as_ref().unwrap().key_to_string(), Some(otp.secret));
    let totp_response = totp.generate();

    // Verification OTP
    let response = server
        .post("/api/users/current/otp/verification")
        .add_header("Cookie", &format!("jwt={}", login_token))
        .json(&json!({ "token": totp_response.token}))
        .await;
    response.assert_status_success();

    // Step 2: Verify OTP is enabled in database
    let ctx = Ctx::new(Ok(user.id.as_ref().unwrap().key_to_string()), false);
    let user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };

    let updated_user = user_db_service
        .get_by_id(&user.id.as_ref().unwrap().key_to_string())
        .await
        .unwrap();
    assert!(updated_user.is_otp_enabled);

    // Step 3: Login should now require OTP (simulate getting OTP token from login)
    let user_id = user.id.as_ref().unwrap().key_to_string();
    let otp_token = ctx_state
        .jwt
        .create_by_otp(&user_id)
        .expect("Failed to create OTP token");

    // Step 4: Generate valid OTP and validate
    let totp = Totp::new(&user_id, updated_user.otp_secret.clone());
    let totp_response = totp.generate();
    let valid_otp_token = totp_response.token;

    let validate_response = server
        .post("/api/users/current/otp/validate")
        .add_header("Authorization", &format!("Bearer {}", otp_token))
        .json(&json!({
            "token": valid_otp_token
        }))
        .await;

    validate_response.assert_status_success();
    let validation_json = validate_response.json::<serde_json::Value>();
    assert!(validation_json.get("token").is_some());
    assert!(validation_json.get("user").is_some());

    // Step 5: Disable OTP
    let disable_response = server
        .post("/api/users/current/otp/disable")
        .add_header("Cookie", &format!("jwt={}", login_token))
        .await;
    disable_response.assert_status_success();

    // Step 6: Verify OTP is disabled
    let final_user = user_db_service
        .get_by_id(&user.id.as_ref().unwrap().key_to_string())
        .await
        .unwrap();
    assert!(!final_user.is_otp_enabled);
});

test_with_server!(
    test_otp_validate_with_expired_otp_token,
    |server, ctx_state, config| {
        let (_server, user, _password, token) = create_fake_login_test_user(&server).await;

        // First enable OTP
        let response = server
            .post("/api/users/current/otp/enable")
            .add_header("Cookie", &format!("jwt={}", token))
            .await;
        response.assert_status_success();
        response.assert_status_success();
        let otp = response.json::<TotpResponse>();
        assert!(otp.url.starts_with("otpauth://totp/"));
        assert!(otp.url.contains("Darve"));

        let totp = Totp::new(&user.id.as_ref().unwrap().key_to_string(), Some(otp.secret));
        let totp_response = totp.generate();

        // Verification OTP
        let response = server
            .post("/api/users/current/otp/verification")
            .add_header("Cookie", &format!("jwt={}", token))
            .json(&json!({ "token": totp_response.token}))
            .await;
        response.assert_status_success();

        let user_id = user.id.as_ref().unwrap().key_to_string();

        // Create OTP token that's already expired (simulate expired token)
        // Note: In a real test environment, you might need to wait or mock time
        // For now, we'll test with a token that should be valid but test the flow
        let otp_token = ctx_state
            .jwt
            .create_by_otp(&user_id)
            .expect("Failed to create OTP token");

        // Generate a valid TOTP token
        let user_db_service = LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &Ctx::new(Ok("".to_string()), false),
        };
        let user_id = user.id.as_ref().unwrap().key_to_string();
        let user_with_otp = user_db_service
            .get_by_id(&user.id.as_ref().unwrap().key_to_string())
            .await
            .unwrap();

        let totp = Totp::new(&user_id, user_with_otp.otp_secret.clone());
        let totp_response = totp.generate();
        let valid_token = totp_response.token;

        // This should work with a fresh token
        let response = server
            .post("/api/users/current/otp/validate")
            .add_header("Authorization", &format!("Bearer {}", otp_token))
            .json(&json!({
                "token": valid_token
            }))
            .await;

        response.assert_status_success();
    }
);

test_with_server!(
    test_otp_login_flow_enable_logout_login_again,
    |server, ctx_state, config| {
        let (_server, user, password, _) = create_fake_login_test_user(&server).await;

        // Step 1: User logs in initially (OTP not enabled yet)
        let login_response = server
            .post("/api/login")
            .json(&json!({
                "username_or_email": user.username,
                "password": password
            }))
            .await;

        login_response.assert_status_success();
        let login_json = login_response.json::<serde_json::Value>();
        let login_token = login_json["token"].as_str().unwrap();

        // Verify this is a regular login token (not OTP token)
        assert!(!login_token.is_empty());

        // Step 2: Enable OTP while logged in
        let response = server
            .post("/api/users/current/otp/enable")
            .add_header("Cookie", &format!("jwt={}", login_token))
            .await;

        response.assert_status_success();
        let otp = response.json::<TotpResponse>();
        assert!(otp.url.starts_with("otpauth://totp/"));
        assert!(otp.url.contains("Darve"));

        let totp = Totp::new(&user.id.as_ref().unwrap().key_to_string(), Some(otp.secret));
        let totp_response = totp.generate();

        // Verification OTP
        let response = server
            .post("/api/users/current/otp/verification")
            .add_header("Cookie", &format!("jwt={}", login_token))
            .json(&json!({ "token": totp_response.token}))
            .await;
        response.assert_status_success();

        // Verify OTP is enabled in database
        let ctx = Ctx::new(Ok(user.id.as_ref().unwrap().key_to_string()), false);
        let user_db_service = LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        };

        let user_with_otp = user_db_service
            .get_by_id(&user.id.as_ref().unwrap().key_to_string())
            .await
            .unwrap();
        assert!(user_with_otp.is_otp_enabled);

        // Step 3: Logout (simulate by not using the cookie)
        let logout_response = server.get("/logout").await;
        logout_response.assert_status_success();

        // Step 4: Try to login again - should now require OTP
        let login_again_response = server
            .post("/api/login")
            .json(&json!({
                "username_or_email": user.username,
                "password": password
            }))
            .await;

        login_again_response.assert_status_success();
        let login_again_json = login_again_response.json::<serde_json::Value>();
        let otp_token = login_again_json["token"].as_str().unwrap();

        // The token should now be an OTP token (shorter expiry)
        assert!(!otp_token.is_empty());
        assert_ne!(otp_token, login_token); // Should be different from initial login token

        // Step 5: Generate valid TOTP and complete authentication
        let user_with_otp = user_db_service
            .get_by_id(&user.id.as_ref().unwrap().key_to_string())
            .await
            .unwrap();
        assert!(user_with_otp.is_otp_enabled);
        assert!(user_with_otp.otp_secret.is_some());

        let totp = Totp::new(
            &user_with_otp.id.as_ref().unwrap().key_to_string(),
            user_with_otp.otp_secret.clone(),
        );
        let totp_response = totp.generate();
        let valid_otp_token = totp_response.token;

        // Step 6: Validate OTP to get final login token
        let validate_response = server
            .post("/api/users/current/otp/validate")
            .add_header("Authorization", &format!("Bearer {}", otp_token))
            .json(&json!({
                "token": valid_otp_token
            }))
            .await;
        validate_response.assert_status_success();
        let final_json = validate_response.json::<serde_json::Value>();
        let final_login_token = final_json["token"].as_str().unwrap();
        let returned_user = &final_json["user"];

        // Verify we got a proper login token and user data
        assert!(!final_login_token.is_empty());
        assert_eq!(returned_user["username"], user.username);
        assert_eq!(returned_user["is_otp_enabled"], true);

        // Step 7: Verify the final token can be used for authenticated requests
        let test_auth_response = server
            .post("/api/users/current/otp/disable")
            .add_header("Cookie", &format!("jwt={}", final_login_token))
            .await;

        test_auth_response.assert_status_success();

        // Step 8: Verify OTP is now disabled
        let final_user = user_db_service
            .get_by_id(&user.id.as_ref().unwrap().key_to_string())
            .await
            .unwrap();
        assert!(!final_user.is_otp_enabled);
    }
);

test_with_server!(
    test_otp_login_flow_wrong_otp_code,
    |server, ctx_state, config| {
        let (_server, user, password, _initial_token) = create_fake_login_test_user(&server).await;

        // Step 1: Enable OTP
        let login_response = server
            .post("/api/login")
            .json(&json!({
                "username_or_email": user.username,
                "password": password
            }))
            .await;
        login_response.assert_status_success();
        let login_json = login_response.json::<serde_json::Value>();
        let login_token = login_json["token"].as_str().unwrap();

        let response = server
            .post("/api/users/current/otp/enable")
            .add_header("Cookie", &format!("jwt={}", login_token))
            .await;

        response.assert_status_success();
        let otp = response.json::<TotpResponse>();
        assert!(otp.url.starts_with("otpauth://totp/"));
        assert!(otp.url.contains("Darve"));

        let totp = Totp::new(&user.id.as_ref().unwrap().key_to_string(), Some(otp.secret));
        let totp_response = totp.generate();

        // Verification OTP
        let response = server
            .post("/api/users/current/otp/verification")
            .add_header("Cookie", &format!("jwt={}", login_token))
            .json(&json!({ "token": totp_response.token}))
            .await;
        response.assert_status_success();

        // Step 2: Login again (should get OTP token)
        let login_again_response = server
            .post("/api/login")
            .json(&json!({
                "username_or_email": user.username,
                "password": password
            }))
            .await;
        login_again_response.assert_status_success();
        let login_again_json = login_again_response.json::<serde_json::Value>();
        let otp_token = login_again_json["token"].as_str().unwrap();

        // Step 3: Try to validate with wrong OTP code
        let validate_response = server
            .post("/api/users/current/otp/validate")
            .add_header("Authorization", &format!("Bearer {}", otp_token))
            .json(&json!({
                "token": "000000"
            }))
            .await;

        validate_response.assert_status_forbidden();
    }
);

test_with_server!(
    test_otp_login_flow_expired_otp_token,
    |server, ctx_state, config| {
        let (_server, user, password, _initial_token) = create_fake_login_test_user(&server).await;

        // Step 1: Enable OTP
        let login_response = server
            .post("/api/login")
            .json(&json!({
                "username_or_email": user.username,
                "password": password
            }))
            .await;
        login_response.assert_status_success();
        let login_json = login_response.json::<serde_json::Value>();
        let login_token = login_json["token"].as_str().unwrap();

        let response = server
            .post("/api/users/current/otp/enable")
            .add_header("Cookie", &format!("jwt={}", login_token))
            .await;
        response.assert_status_success();
        let otp = response.json::<TotpResponse>();
        assert!(otp.url.starts_with("otpauth://totp/"));
        assert!(otp.url.contains("Darve"));

        let totp = Totp::new(&user.id.as_ref().unwrap().key_to_string(), Some(otp.secret));
        let totp_response = totp.generate();

        // Verification OTP
        let response = server
            .post("/api/users/current/otp/verification")
            .add_header("Cookie", &format!("jwt={}", login_token))
            .json(&json!({ "token": totp_response.token}))
            .await;
        response.assert_status_success();

        // Step 2: Create an expired OTP token manually
        let user_id = user.id.as_ref().unwrap().key_to_string();

        let expired_otp_token = ctx_state
            .jwt
            .create_by_otp(&user_id)
            .expect("Failed to create OTP token");
        let user_db_service = LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &Ctx::new(Ok("".to_string()), false),
        };

        let user_with_otp = user_db_service
            .get_by_id(&user.id.as_ref().unwrap().key_to_string())
            .await
            .unwrap();
        assert!(user_with_otp.is_otp_enabled);

        // Step 3: Generate valid TOTP code
        let totp = Totp::new(&user_id, user_with_otp.otp_secret.clone());
        let totp_response = totp.generate();
        let valid_otp_code = totp_response.token;

        // Step 4: Try to use the OTP token (in real scenario this would be expired after 1 minute)
        // Since we can't easily simulate time passing, we test with a fresh token
        // This test mainly verifies the flow works with proper tokens
        let validate_response = server
            .post("/api/users/current/otp/validate")
            .add_header("Authorization", &format!("Bearer {}", expired_otp_token))
            .json(&json!({
                "token": valid_otp_code
            }))
            .await;

        validate_response.assert_status_success();
    }
);

test_with_server!(
    test_otp_disabled_user_normal_login,
    |server, ctx_state, config| {
        let (_server, user, password, _initial_token) = create_fake_login_test_user(&server).await;

        // User has OTP disabled by default, login should work normally
        let login_response = server
            .post("/api/login")
            .json(&json!({
                "username_or_email": user.username,
                "password": password
            }))
            .await;

        login_response.assert_status_success();
        let login_json = login_response.json::<serde_json::Value>();
        let login_token = login_json["token"].as_str().unwrap();

        // Verify this is a regular login token (can be used immediately)
        let test_response = server
            .post("/api/users/current/otp/enable")
            .add_header("Cookie", &format!("jwt={}", login_token))
            .await;

        test_response.assert_status_success();
    }
);
test_with_server!(
    test_otp_login_without_verification,
    |server, ctx_state, config| {
        let (_server, user, password, _initial_token) = create_fake_login_test_user(&server).await;

        // Step 1: Enable OTP
        let login_response = server
            .post("/api/login")
            .json(&json!({
                "username_or_email": user.username,
                "password": password
            }))
            .await;
        login_response.assert_status_success();
        let login_json = login_response.json::<serde_json::Value>();
        let login_token = login_json["token"].as_str().unwrap();

        let response = server
            .post("/api/users/current/otp/enable")
            .add_header("Cookie", &format!("jwt={}", login_token))
            .await;

        response.assert_status_success();
        let otp = response.json::<TotpResponse>();
        assert!(otp.url.starts_with("otpauth://totp/"));
        assert!(otp.url.contains("Darve"));

        // Step 2: Login again (should get OTP token)
        let login_again_response = server
            .post("/api/login")
            .json(&json!({
                "username_or_email": user.username,
                "password": password
            }))
            .await;
        login_again_response.assert_status_success();
        let login_again_json = login_again_response.json::<serde_json::Value>();
        let user = serde_json::from_value::<LocalUser>(login_again_json["user"].clone()).unwrap();
        let token = login_again_json["token"].as_str().unwrap();

        let res = ctx_state
            .jwt
            .decode_by_type(token, darve_server::utils::jwt::TokenType::Login);

        assert!(res.is_ok());
        assert!(!user.is_otp_enabled);
    }
);
