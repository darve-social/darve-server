mod helpers;

use darve_server::entities::nickname::Nickname;
use helpers::{create_fake_login_test_user, create_login_test_user};
use serde_json::json;

test_with_server!(test_get_nicknames_empty, |server, ctx_state, config| {
    let (server, _user, _pwd, _token) = create_fake_login_test_user(&server).await;

    let response = server
        .get("/api/users/current/nicknames")
        .add_header("Accept", "application/json")
        .await;

    response.assert_status_success();
    let nicknames: Vec<Nickname> = response.json();
    assert!(
        nicknames.is_empty(),
        "Should return empty array for new user"
    );
});

test_with_server!(test_set_and_get_nickname, |server, ctx_state, config| {
    // Create two users
    let (server, user2, _, _) = create_fake_login_test_user(&server).await;
    let (server, _user1, _pwd1, _token1) = create_fake_login_test_user(&server).await;
    let user2_id = user2.id.as_ref().unwrap().id.to_raw();

    // Set a nickname for user2 by user1
    let nickname_data = json!({
        "nickname": "My Friend"
    });

    let response = server
        .post(&format!("/api/users/{}/nickname", user2_id))
        .json(&nickname_data)
        .add_header("Accept", "application/json")
        .await;

    response.assert_status_success();

    // Get nicknames for current user (user1)
    let response = server
        .get("/api/users/current/nicknames")
        .add_header("Accept", "application/json")
        .await;

    response.assert_status_success();
    let nicknames: Vec<Nickname> = response.json();
    assert_eq!(nicknames.len(), 1, "Should have one nickname");
});

test_with_server!(test_update_nickname, |server, ctx_state, config| {
    // Create two users
    let (server, user2, _, _) = create_fake_login_test_user(&server).await;
    let (server, _user1, _pwd1, _token1) = create_fake_login_test_user(&server).await;

    let user2_id = user2.id.as_ref().unwrap().id.to_raw();
    // Set initial nickname
    let nickname_data = json!({
        "nickname": "Initial Nickname"
    });

    let response = server
        .post(&format!("/api/users/{}/nickname", user2_id))
        .json(&nickname_data)
        .add_header("Accept", "application/json")
        .await;

    response.assert_status_success();

    // Update nickname
    let updated_nickname_data = json!({
        "nickname": "Updated Nickname"
    });

    let response = server
        .post(&format!("/api/users/{}/nickname", user2_id))
        .json(&updated_nickname_data)
        .add_header("Accept", "application/json")
        .await;

    response.assert_status_success();

    // Verify updated nickname
    let response = server
        .get("/api/users/current/nicknames")
        .add_header("Accept", "application/json")
        .await;

    response.assert_status_success();
    let nicknames: Vec<Nickname> = response.json();
    assert_eq!(nicknames.len(), 1, "Should still have one nickname");
});

test_with_server!(test_remove_nickname, |server, ctx_state, config| {
    // Create two users
    let (server, user2, _, _) = create_fake_login_test_user(&server).await;
    let (server, _user1, _pwd1, _token1) = create_fake_login_test_user(&server).await;

    let user2_id = user2.id.as_ref().unwrap().id.to_raw();
    // Set a nickname first
    let nickname_data = json!({
        "nickname": "Temporary Nickname"
    });

    let response = server
        .post(&format!("/api/users/{}/nickname", user2_id))
        .json(&nickname_data)
        .add_header("Accept", "application/json")
        .await;

    response.assert_status_success();

    // Verify nickname exists
    let response = server
        .get("/api/users/current/nicknames")
        .add_header("Accept", "application/json")
        .await;

    response.assert_status_success();
    let nicknames: Vec<Nickname> = response.json();
    assert_eq!(
        nicknames.len(),
        1,
        "Should have one nickname before removal"
    );

    // Remove nickname by setting it to null
    let remove_data = json!({
        "nickname": null
    });

    let response = server
        .post(&format!("/api/users/{}/nickname", user2_id))
        .json(&remove_data)
        .add_header("Accept", "application/json")
        .await;

    response.assert_status_success();

    // Verify nickname is removed
    let response = server
        .get("/api/users/current/nicknames")
        .add_header("Accept", "application/json")
        .await;

    response.assert_status_success();
    let nicknames: Vec<Nickname> = response.json();
    assert!(
        nicknames.is_empty(),
        "Should have no nicknames after removal"
    );
});

test_with_server!(
    test_set_empty_nickname_error,
    |server, ctx_state, config| {
        // Create two users
        let (server, user2_id) = create_login_test_user(&server, "testuser2".to_string()).await;
        let (server, _user1, _pwd1, _token1) = create_fake_login_test_user(&server).await;

        // Try to set empty nickname
        let nickname_data = json!({
            "nickname": ""
        });

        let response = server
            .post(&format!("/api/users/{}/nickname", user2_id))
            .json(&nickname_data)
            .add_header("Accept", "application/json")
            .await;

        response.assert_status_bad_request();

        // Try with whitespace only
        let nickname_data = json!({
            "nickname": "   "
        });

        let response = server
            .post(&format!("/api/users/{}/nickname", user2_id))
            .json(&nickname_data)
            .add_header("Accept", "application/json")
            .await;

        response.assert_status_bad_request();
    }
);

test_with_server!(
    test_set_nickname_invalid_user_id,
    |server, ctx_state, config| {
        let (server, _user1, _pwd1, _token1) = create_fake_login_test_user(&server).await;

        // Try to set nickname for non-existent user
        let nickname_data = json!({
            "nickname": "Some Nickname"
        });

        let response = server
            .post("/api/users/invalid_user_id/nickname")
            .json(&nickname_data)
            .add_header("Accept", "application/json")
            .await;

        response.assert_status_failure();
    }
);

test_with_server!(test_multiple_nicknames, |server, ctx_state, config| {
    // Create multiple users
    let (server, user2, _, _) = create_fake_login_test_user(&server).await;
    let (server, user3, _, _) = create_fake_login_test_user(&server).await;
    let (server, user4, _, _) = create_fake_login_test_user(&server).await;
    let (server, _user1, _pwd1, _token1) = create_fake_login_test_user(&server).await;

    let user2_id = user2.id.as_ref().unwrap().id.to_raw();
    let user3_id = user3.id.as_ref().unwrap().id.to_raw();
    let user4_id = user4.id.as_ref().unwrap().id.to_raw();

    // Set nicknames for multiple users
    let nickname_data_2 = json!({
        "nickname": "Best Friend"
    });
    let nickname_data_3 = json!({
        "nickname": "Colleague"
    });
    let nickname_data_4 = json!({
        "nickname": "Neighbor"
    });

    let response = server
        .post(&format!("/api/users/{}/nickname", user2_id))
        .json(&nickname_data_2)
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    let response = server
        .post(&format!("/api/users/{}/nickname", user3_id))
        .json(&nickname_data_3)
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    let response = server
        .post(&format!("/api/users/{}/nickname", user4_id))
        .json(&nickname_data_4)
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();

    // Get all nicknames
    let response = server
        .get("/api/users/current/nicknames")
        .add_header("Accept", "application/json")
        .await;

    response.assert_status_success();
    let nicknames: Vec<Nickname> = response.json();
    assert_eq!(nicknames.len(), 3, "Should have three nicknames");
    assert!(nicknames
        .iter()
        .find(|n| n.user_id == user2_id && n.name == "Best Friend")
        .is_some());
    assert!(nicknames
        .iter()
        .find(|n| n.user_id == user3_id && n.name == "Colleague")
        .is_some());
    assert!(nicknames
        .iter()
        .find(|n| n.user_id == user4_id && n.name == "Neighbor")
        .is_some());
});

test_with_server!(
    test_nickname_workflow_integration,
    |server, ctx_state, config| {
        // Create users
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let (server, user3, _, _) = create_fake_login_test_user(&server).await;
        let (server, _user1, _pwd1, _token1) = create_fake_login_test_user(&server).await;

        let user2_id = user2.id.as_ref().unwrap().id.to_raw();
        let user3_id = user3.id.as_ref().unwrap().id.to_raw();
        // Initial state - no nicknames
        let response = server
            .get("/api/users/current/nicknames")
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let nicknames: Vec<Nickname> = response.json();
        assert!(nicknames.is_empty());

        // Set first nickname
        let response = server
            .post(&format!("/api/users/{}/nickname", user2_id))
            .json(&json!({"nickname": "Friend One"}))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();

        // Verify one nickname exists
        let response = server
            .get("/api/users/current/nicknames")
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let nicknames: Vec<Nickname> = response.json();
        assert_eq!(nicknames.len(), 1);

        // Set second nickname
        let response = server
            .post(&format!("/api/users/{}/nickname", user3_id))
            .json(&json!({"nickname": "Friend Two"}))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();

        // Verify two nicknames exist
        let response = server
            .get("/api/users/current/nicknames")
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let nicknames: Vec<Nickname> = response.json();
        assert_eq!(nicknames.len(), 2);

        // Update first nickname
        let response = server
            .post(&format!("/api/users/{}/nickname", user2_id))
            .json(&json!({"nickname": "Updated Friend"}))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();

        // Still should have two nicknames
        let response = server
            .get("/api/users/current/nicknames")
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let nicknames: Vec<Nickname> = response.json();
        assert_eq!(nicknames.len(), 2);

        // Remove one nickname
        let response = server
            .post(&format!("/api/users/{}/nickname", user2_id))
            .json(&json!({"nickname": null}))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();

        // Should have one nickname left
        let response = server
            .get("/api/users/current/nicknames")
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let nicknames: Vec<Nickname> = response.json();
        assert_eq!(nicknames.len(), 1);

        // Remove last nickname
        let response = server
            .post(&format!("/api/users/{}/nickname", user3_id))
            .json(&json!({"nickname": null}))
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();

        // Should have no nicknames
        let response = server
            .get("/api/users/current/nicknames")
            .add_header("Accept", "application/json")
            .await;
        response.assert_status_success();
        let nicknames: Vec<Nickname> = response.json();
        assert!(nicknames.is_empty());
    }
);
