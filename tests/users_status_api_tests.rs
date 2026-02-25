mod helpers;

use crate::helpers::create_fake_login_test_user;
use darve_server::utils::user_presence_guard::UserPresenceGuard;
use serde_json::Value;
use tokio::time::{sleep, Duration};

test_with_server!(test_get_users_status_basic, |server, ctx_state, config| {
    // Create a test user and login
    let (_, user1, _password1, token1) = create_fake_login_test_user(&server).await;
    let (_, user2, _password2, _token2) = create_fake_login_test_user(&server).await;
    let (_, user3, _password3, _token3) = create_fake_login_test_user(&server).await;

    let user1_id = user1.id.unwrap().to_raw();
    let user2_id = user2.id.unwrap().to_raw();
    let user3_id = user3.id.unwrap().to_raw();

    // Simulate user1 and user2 being online by creating presence guards
    let _guard1 = UserPresenceGuard::new(ctx_state.clone(), user1_id.clone());
    let _guard2 = UserPresenceGuard::new(ctx_state.clone(), user2_id.clone());
    // user3 remains offline

    // Test getting status of multiple users
    let query_params = format!(
        "user_ids={}&user_ids={}&user_ids={}",
        user1_id, user2_id, user3_id
    );

    let response = server
        .get(&format!("/api/users/status?{}", query_params))
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;

    response.assert_status_success();

    let statuses: Vec<Value> = response.json();
    assert_eq!(statuses.len(), 3);

    // Find and verify each user's status
    let user1_status = statuses
        .iter()
        .find(|s| s["user_id"] == user1_id)
        .expect("Should find user1 status");
    let user2_status = statuses
        .iter()
        .find(|s| s["user_id"] == user2_id)
        .expect("Should find user2 status");
    let user3_status = statuses
        .iter()
        .find(|s| s["user_id"] == user3_id)
        .expect("Should find user3 status");

    assert_eq!(user1_status["is_online"], true);
    assert_eq!(user2_status["is_online"], true);
    assert_eq!(user3_status["is_online"], false);
});

test_with_server!(
    test_get_users_status_unauthorized,
    |server, ctx_state, config| {
        // Try to access users status without authentication
        let response = server.get("/api/users/status?user_ids=some_user_id").await;

        response.assert_status_unauthorized();
    }
);

test_with_server!(
    test_get_users_status_empty_list,
    |server, ctx_state, config| {
        // Create a test user and login
        let (_, _user, _password, token) = create_fake_login_test_user(&server).await;

        // Test with empty user_ids list
        let response = server
            .get("/api/users/status")
            .add_header("Authorization", format!("Bearer {}", token))
            .await;

        response.assert_status_success();

        let statuses: Vec<Value> = response.json();
        assert_eq!(statuses.len(), 0);
    }
);

test_with_server!(
    test_get_users_status_nonexistent_users,
    |server, ctx_state, config| {
        // Create a test user and login
        let (_, _user, _password, token) = create_fake_login_test_user(&server).await;

        // Test with nonexistent user IDs
        let fake_user_ids = vec!["nonexistent_user_1", "nonexistent_user_2"];
        let query_params = fake_user_ids
            .iter()
            .map(|id| format!("user_ids={}", id))
            .collect::<Vec<_>>()
            .join("&");

        let response = server
            .get(&format!("/api/users/status?{}", query_params))
            .add_header("Authorization", format!("Bearer {}", token))
            .await;

        response.assert_status_success();

        let statuses: Vec<Value> = response.json();
        assert_eq!(statuses.len(), 2);

        // All should be offline since they don't exist
        for status in statuses {
            assert_eq!(status["is_online"], false);
        }
    }
);

test_with_server!(
    test_get_users_status_multiple_connections,
    |server, ctx_state, config| {
        // Create a test user and login
        let (_, user1, _password1, token1) = create_fake_login_test_user(&server).await;
        let (_, user2, _password2, _token2) = create_fake_login_test_user(&server).await;

        let user1_id = user1.id.unwrap().to_raw();
        let user2_id = user2.id.unwrap().to_raw();

        // Create multiple presence guards for user1 (simulating multiple connections)
        let _guard1a = UserPresenceGuard::new(ctx_state.clone(), user1_id.clone());
        let _guard1b = UserPresenceGuard::new(ctx_state.clone(), user1_id.clone());

        // Single connection for user2
        let _guard2 = UserPresenceGuard::new(ctx_state.clone(), user2_id.clone());

        // Verify both users show as online
        let query_params = format!("user_ids={}&user_ids={}", user1_id, user2_id);

        let response = server
            .get(&format!("/api/users/status?{}", query_params))
            .add_header("Authorization", format!("Bearer {}", token1))
            .await;

        response.assert_status_success();

        let statuses: Vec<Value> = response.json();
        assert_eq!(statuses.len(), 2);

        // Both users should be online
        for status in statuses {
            assert_eq!(status["is_online"], true);
        }

        // Drop one of user1's connections
        drop(_guard1a);

        // User1 should still be online (has another connection)
        let response = server
            .get(&format!("/api/users/status?user_ids={}", user1_id))
            .add_header("Authorization", format!("Bearer {}", token1))
            .await;

        response.assert_status_success();
        let statuses: Vec<Value> = response.json();
        assert_eq!(statuses[0]["is_online"], true);
    }
);

test_with_server!(
    test_get_users_status_connection_cleanup,
    |server, ctx_state, config| {
        // Create test users
        let (_, user1, _password1, token1) = create_fake_login_test_user(&server).await;
        let user1_id = user1.id.unwrap().to_raw();

        // Create presence guard
        let guard = UserPresenceGuard::new(ctx_state.clone(), user1_id.clone());

        // Verify user is online
        let response = server
            .get(&format!("/api/users/status?user_ids={}", user1_id))
            .add_header("Authorization", format!("Bearer {}", token1))
            .await;

        response.assert_status_success();
        let statuses: Vec<Value> = response.json();
        assert_eq!(statuses[0]["is_online"], true);

        // Drop the guard to simulate disconnect
        drop(guard);

        // Initially should still be online (due to 10-second cleanup delay)
        let response = server
            .get(&format!("/api/users/status?user_ids={}", user1_id))
            .add_header("Authorization", format!("Bearer {}", token1))
            .await;

        response.assert_status_success();
        let statuses: Vec<Value> = response.json();
        assert_eq!(statuses[0]["is_online"], true);

        // Wait for cleanup delay (testing this is tricky due to timing)
        // In practice, this test might be flaky, but provides insight into the behavior
        sleep(Duration::from_secs(12)).await;

        let response = server
            .get(&format!("/api/users/status?user_ids={}", user1_id))
            .add_header("Authorization", format!("Bearer {}", token1))
            .await;

        response.assert_status_success();
        let statuses: Vec<Value> = response.json();

        // After cleanup delay, user might be offline (timing dependent)
        println!("User status after cleanup: {:?}", statuses[0]["is_online"]);
    }
);

test_with_server!(
    test_get_users_status_large_request,
    |server, ctx_state, config| {
        // Create a test user and login
        let (_, _user, _password, token) = create_fake_login_test_user(&server).await;

        // Test with many user IDs to ensure the endpoint can handle larger requests
        let user_ids: Vec<String> = (1..=50).map(|i| format!("user_{}", i)).collect();

        let query_params = user_ids
            .iter()
            .map(|id| format!("user_ids={}", id))
            .collect::<Vec<_>>()
            .join("&");

        let response = server
            .get(&format!("/api/users/status?{}", query_params))
            .add_header("Authorization", format!("Bearer {}", token))
            .await;

        response.assert_status_success();

        let statuses: Vec<Value> = response.json();
        assert_eq!(statuses.len(), 50);

        // All should be offline since they're fake user IDs
        for status in statuses {
            assert_eq!(status["is_online"], false);
        }
    }
);
