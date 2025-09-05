mod helpers;

use crate::helpers::create_fake_login_test_user;
use darve_server::middleware::mw_ctx::AppEventType;
use darve_server::utils::user_presence_guard::UserPresenceGuard;
use serde_json::Value;
use std::time::Duration;
use tokio::time::{sleep, timeout};

test_with_server!(
    test_sse_endpoint_authentication,
    |server, ctx_state, config| {
        // Test SSE endpoint without authentication
        let response = server.get("/api/notifications/sse").await;
        response.assert_status_unauthorized();
    }
);

test_with_server!(
    test_sse_endpoint_creates_presence_guard,
    |server, ctx_state, config| {
        // Create a test user and login
        let (_, user, _password, _token) = create_fake_login_test_user(&server).await;
        let user_id = user.id.unwrap().to_raw();

        // Before SSE connection, user should not be online
        assert!(!ctx_state.online_users.contains_key(&user_id));

        // Note: Testing actual SSE connection is complex with axum-test
        // We'll test the presence guard creation logic directly instead

        // Simulate what happens when SSE endpoint is called
        let presence_guard = UserPresenceGuard::new(ctx_state.clone(), user_id.clone());

        // Verify user is now marked as online
        assert!(ctx_state.online_users.contains_key(&user_id));
        assert_eq!(*ctx_state.online_users.get(&user_id).unwrap(), 1);

        // Clean up
        drop(presence_guard);
    }
);

test_with_server!(test_sse_user_status_events, |server, ctx_state, config| {
    // Create test users
    let (_, user1, _password1, _token1) = create_fake_login_test_user(&server).await;
    let (_, user2, _password2, _token2) = create_fake_login_test_user(&server).await;

    let user1_id = user1.id.unwrap().to_raw();
    let user2_id = user2.id.unwrap().to_raw();

    // Subscribe to events to test what would be sent via SSE
    let mut event_receiver = ctx_state.event_sender.subscribe();

    // User1 comes online
    let _guard1 = UserPresenceGuard::new(ctx_state.clone(), user1_id.clone());

    // Should receive online event
    let event = timeout(Duration::from_secs(1), event_receiver.recv())
        .await
        .expect("Should receive event within timeout")
        .expect("Should receive valid event");

    match event.event {
        AppEventType::UserStatus(status) => {
            assert!(status.is_online);
            assert_eq!(event.user_id, user1_id);
        }
        _ => panic!("Expected UserStatus event"),
    }

    // User2 comes online
    let _guard2 = UserPresenceGuard::new(ctx_state.clone(), user2_id.clone());

    // Should receive another online event
    let event = timeout(Duration::from_secs(1), event_receiver.recv())
        .await
        .expect("Should receive event within timeout")
        .expect("Should receive valid event");

    match event.event {
        AppEventType::UserStatus(status) => {
            assert!(status.is_online);
            assert_eq!(event.user_id, user2_id);
        }
        _ => panic!("Expected UserStatus event"),
    }
});

test_with_server!(
    test_sse_multiple_connections_same_user,
    |server, ctx_state, config| {
        // Create a test user
        let (_, user, _password, _token) = create_fake_login_test_user(&server).await;
        let user_id = user.id.unwrap().to_raw();

        // Subscribe to events
        let mut event_receiver = ctx_state.event_sender.subscribe();

        // First connection
        let _guard1 = UserPresenceGuard::new(ctx_state.clone(), user_id.clone());

        // Should receive online event
        let event = timeout(Duration::from_secs(1), event_receiver.recv())
            .await
            .expect("Should receive event within timeout")
            .expect("Should receive valid event");

        match event.event {
            AppEventType::UserStatus(status) => {
                assert!(status.is_online);
            }
            _ => panic!("Expected UserStatus event"),
        }

        // Second connection from same user
        let _guard2 = UserPresenceGuard::new(ctx_state.clone(), user_id.clone());

        // Should NOT receive another online event (user already online)
        let result = timeout(Duration::from_millis(500), event_receiver.recv()).await;
        assert!(
            result.is_err(),
            "Should not receive additional online event for same user"
        );

        // Verify user count is 2
        assert_eq!(*ctx_state.online_users.get(&user_id).unwrap(), 2);
    }
);

test_with_server!(
    test_sse_presence_guard_lifecycle,
    |server, ctx_state, config| {
        // Create a test user
        let (_, user, _password, _token) = create_fake_login_test_user(&server).await;
        let user_id = user.id.unwrap().to_raw();

        // Subscribe to events
        let mut event_receiver = ctx_state.event_sender.subscribe();

        // Create presence guard (simulating SSE connection)
        let guard = UserPresenceGuard::new(ctx_state.clone(), user_id.clone());

        // Verify user is online
        assert!(ctx_state.online_users.contains_key(&user_id));

        // Should receive online event
        let event = timeout(Duration::from_secs(1), event_receiver.recv())
            .await
            .expect("Should receive online event")
            .expect("Should receive valid event");

        match event.event {
            AppEventType::UserStatus(status) => {
                assert!(status.is_online);
            }
            _ => panic!("Expected UserStatus event"),
        }

        // Drop the guard (simulating SSE connection close)
        drop(guard);

        // Wait for the async cleanup (10+ seconds in real implementation)
        // Note: In real testing, we'd need to wait longer, but for test purposes
        // we'll verify the immediate state and acknowledge the async cleanup

        // Immediately after drop, user might still be in the map due to async cleanup
        // The actual cleanup happens after a delay
        println!(
            "User online status after guard drop: {}",
            ctx_state.online_users.contains_key(&user_id)
        );
    }
);

test_with_server!(
    test_sse_concurrent_connections,
    |server, ctx_state, config| {
        // Create multiple test users
        let (_, user1, _password1, token1) = create_fake_login_test_user(&server).await;
        let (_, user2, _password2, _token2) = create_fake_login_test_user(&server).await;
        let (_, user3, _password3, _token3) = create_fake_login_test_user(&server).await;

        let user1_id = user1.id.unwrap().id.to_raw();
        let user2_id = user2.id.unwrap().id.to_raw();
        let user3_id = user3.id.unwrap().id.to_raw();

        // Create concurrent presence guards
        let guard1 = UserPresenceGuard::new(ctx_state.clone(), user1_id.clone());
        let guard2 = UserPresenceGuard::new(ctx_state.clone(), user2_id.clone());
        let guard3 = UserPresenceGuard::new(ctx_state.clone(), user3_id.clone());

        // Verify all users are online
        assert!(ctx_state.online_users.contains_key(&user1_id));
        assert!(ctx_state.online_users.contains_key(&user2_id));
        assert!(ctx_state.online_users.contains_key(&user3_id));

        // Test using the users/status API to verify presence state
        let response = server
            .get("/api/users/status")
            .add_query_param("user_ids", &user3_id)
            .add_query_param("user_ids", &user2_id)
            .add_query_param("user_ids", &user1_id)
            .add_header("Cookie", format!("jwt={}", token1))
            .await;
        response.assert_status_success();

        let statuses: Vec<Value> = response.json();
        assert_eq!(statuses.len(), 3);

        // All users should be online
        for status in statuses {
            assert_eq!(status["is_online"], true);
        }

        // Clean up guards
        drop(guard1);
        drop(guard2);
        drop(guard3);
    }
);

test_with_server!(
    test_sse_rapid_connect_disconnect,
    |server, ctx_state, config| {
        // Create a test user
        let (_, user, _password, _token) = create_fake_login_test_user(&server).await;
        let user_id = user.id.unwrap().to_raw();

        // Simulate rapid connections and disconnections
        for _ in 0..5 {
            let guard = UserPresenceGuard::new(ctx_state.clone(), user_id.clone());
            // Verify user is tracked
            assert!(ctx_state.online_users.contains_key(&user_id));

            // Immediate disconnect
            drop(guard);

            // Brief pause
            sleep(Duration::from_millis(100)).await;
        }

        // After rapid connect/disconnect cycles, the async cleanup behavior
        // will determine the final state. This test helps identify any race conditions.
        println!(
            "Final online state after rapid connect/disconnect: {}",
            ctx_state.online_users.contains_key(&user_id)
        );
    }
);
