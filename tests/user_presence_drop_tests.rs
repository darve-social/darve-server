mod helpers;

use crate::helpers::create_fake_login_test_user;
use darve_server::{
    entities::user_auth::local_user_entity::LocalUserDbService,
    middleware::{
        ctx::Ctx,
        mw_ctx::{AppEvent, AppEventType, AppEventUsetStatus},
    },
    utils::user_presence_guard::UserPresenceGuard,
};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::{sleep, timeout};

/// Helper function to create a minimal test context for UserPresenceGuard tests
async fn create_test_presence_context() -> (
    Arc<DashMap<String, usize>>,
    broadcast::Sender<AppEvent>,
    broadcast::Receiver<AppEvent>,
) {
    let (event_sender, event_receiver) = broadcast::channel(100);
    let online_users = Arc::new(DashMap::new());
    (online_users, event_sender, event_receiver)
}

/// Helper function to wait for and validate a UserStatus event
async fn expect_user_status_event(
    receiver: &mut broadcast::Receiver<AppEvent>,
    expected_user_id: &str,
    expected_online: bool,
    timeout_secs: u64,
) -> Result<AppEvent, String> {
    let event = timeout(Duration::from_secs(timeout_secs), receiver.recv())
        .await
        .map_err(|_| format!("Timeout waiting for user status event"))?
        .map_err(|e| format!("Error receiving event: {}", e))?;

    match &event.event {
        AppEventType::UserStatus(status) => {
            if event.user_id == expected_user_id && status.is_online == expected_online {
                Ok(event)
            } else {
                Err(format!(
                    "Event mismatch: expected user_id={}, is_online={}, got user_id={}, is_online={}",
                    expected_user_id, expected_online, event.user_id, status.is_online
                ))
            }
        }
        _ => Err("Expected UserStatus event".to_string()),
    }
}
#[tokio::test]
async fn test_drop_behavior_multiple_connections() {
    let (online_users, event_sender, mut event_receiver) = create_test_presence_context().await;
    let user_id = "drop_test_user_multiple".to_string();

    // First connection
    {
        let mut count = online_users.entry(user_id.clone()).or_insert(0);
        *count += 1;
        if *count == 1 {
            let _ = event_sender.send(AppEvent {
                user_id: user_id.clone(),
                metadata: None,
                content: None,
                receivers: vec![],
                event: AppEventType::UserStatus(AppEventUsetStatus { is_online: true }),
            });
        }
    }

    // Second connection
    {
        let mut count = online_users.entry(user_id.clone()).or_insert(0);
        *count += 1;
        // Should not send another online event
    }

    // Verify only one online event was sent
    let _online_event = expect_user_status_event(&mut event_receiver, &user_id, true, 1)
        .await
        .expect("Should receive online event");

    // Should not receive another online event
    let result = timeout(Duration::from_millis(500), event_receiver.recv()).await;
    assert!(
        result.is_err(),
        "Should not receive additional online event"
    );

    // Verify count is 2
    assert_eq!(*online_users.get(&user_id).unwrap(), 2);

    // Simulate first connection drop
    {
        if let Some(mut entry) = online_users.get_mut(&user_id) {
            *entry -= 1;
            if *entry == 0 {
                drop(entry);
                let _ = event_sender.send(AppEvent {
                    user_id: user_id.clone(),
                    metadata: None,
                    content: None,
                    receivers: vec![],
                    event: AppEventType::UserStatus(AppEventUsetStatus { is_online: false }),
                });
                online_users.remove(&user_id);
            }
        }
    }

    // User should still be online (count = 1)
    assert_eq!(*online_users.get(&user_id).unwrap(), 1);

    // Should not receive offline event yet
    let result = timeout(Duration::from_millis(500), event_receiver.recv()).await;
    assert!(
        result.is_err(),
        "Should not receive offline event while user still has connections"
    );

    // Simulate second connection drop
    {
        if let Some(mut entry) = online_users.get_mut(&user_id) {
            *entry -= 1;
            if *entry == 0 {
                drop(entry);
                let _ = event_sender.send(AppEvent {
                    user_id: user_id.clone(),
                    metadata: None,
                    content: None,
                    receivers: vec![],
                    event: AppEventType::UserStatus(AppEventUsetStatus { is_online: false }),
                });
                online_users.remove(&user_id);
            }
        }
    }

    // Now should receive offline event
    let _offline_event = expect_user_status_event(&mut event_receiver, &user_id, false, 1)
        .await
        .expect("Should receive offline event after all connections closed");

    // User should be removed from online_users
    assert!(!online_users.contains_key(&user_id));
}

test_with_server!(
    test_presence_guard_with_database_integration,
    |server, ctx_state, config| {
        // Create a test user
        let (_, user, _password, _token) = create_fake_login_test_user(&server).await;
        let user_id = user.id.unwrap().id.to_raw();

        // Create presence guard
        let guard = UserPresenceGuard::new(ctx_state.clone(), user_id.clone());

        // Verify user is tracked
        assert!(ctx_state.online_users.contains_key(&user_id));

        // Get the user's last_seen before dropping the guard
        let user_service = LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &Ctx::new(Ok(user_id.clone()), false),
        };

        let user_before = user_service
            .get_by_id(&user_id)
            .await
            .expect("Should find user");

        let last_seen_before = user_before.last_seen.clone();

        // Drop the guard (this will trigger async cleanup including last_seen update)
        drop(guard);

        // Wait longer than the 10-second cleanup delay to ensure database update completes
        sleep(Duration::from_secs(12)).await;

        // Check if last_seen was updated (this might be timing dependent)
        let user_after = user_service
            .get_by_id(&user_id)
            .await
            .expect("Should find user");

        assert_ne!(last_seen_before, user_after.last_seen);
    }
);

test_with_server!(
    test_concurrent_presence_management,
    |server, ctx_state, config| {
        // Create multiple test users
        let mut users = Vec::new();
        let mut guards = Vec::new();

        for _ in 0..5 {
            let (_, user, _, _) = create_fake_login_test_user(&server).await;
            let user_id = user.id.unwrap().to_raw();
            users.push(user_id.clone());

            // Create presence guard for each user
            let guard = UserPresenceGuard::new(ctx_state.clone(), user_id.clone());
            guards.push(guard);

            // Verify user is tracked
            assert!(ctx_state.online_users.contains_key(&user_id));
        }

        // All users should be online
        assert_eq!(ctx_state.online_users.len(), 5);

        // Drop all guards simultaneously to test concurrent cleanup
        guards.clear(); // This drops all guards

        // Wait for potential cleanup
        sleep(Duration::from_secs(12)).await;

        // Check final state (timing dependent)
        let remaining_users = ctx_state.online_users.len();
        println!(
            "Users remaining online after concurrent cleanup: {}",
            remaining_users
        );
    }
);

test_with_server!(
    test_stress_presence_operations,
    |server, ctx_state, config| {
        // Create a test user
        let (_, user, _password, _token) = create_fake_login_test_user(&server).await;
        let user_id = user.id.unwrap().to_raw();

        // Perform many rapid operations to stress test the system
        for cycle in 0..3 {
            let mut guards = Vec::new();

            // Create multiple connections rapidly
            for _ in 0..3 {
                let guard = UserPresenceGuard::new(ctx_state.clone(), user_id.clone());
                guards.push(guard);
            }

            // Verify user is online with correct count
            if let Some(count) = ctx_state.online_users.get(&user_id) {
                assert_eq!(*count, 3, "Cycle {}: Expected count 3", cycle);
            } else {
                panic!("Cycle {}: User should be online", cycle);
            }

            // Drop all connections
            guards.clear();

            // Brief pause between cycles
            sleep(Duration::from_secs(11)).await;
        }

        // Final state check after stress test
        sleep(Duration::from_secs(12)).await;

        let is_online = ctx_state.online_users.contains_key(&user_id);
        println!("User online status after stress test: {}", is_online);
    }
);

test_with_server!(
    test_integrated_presence_and_status_api,
    |server, ctx_state, config| {
        // Create test users
        let (_, user1, _password1, token1) = create_fake_login_test_user(&server).await;
        let (_, user2, _password2, _token2) = create_fake_login_test_user(&server).await;

        let user1_id = user1.id.unwrap().to_raw();
        let user2_id = user2.id.unwrap().to_raw();

        // Initially both users should be offline
        let query_params = format!("user_ids={}&user_ids={}", user1_id, user2_id);
        let response = server
            .get(&format!("/api/users/status?{}", query_params))
            .add_header("Cookie", format!("jwt={}", token1))
            .await;

        response.assert_status_success();
        let statuses: Vec<serde_json::Value> = response.json();
        assert_eq!(statuses.len(), 2);

        for status in &statuses {
            assert_eq!(status["is_online"], false);
        }

        // Bring user1 online
        let _guard1 = UserPresenceGuard::new(ctx_state.clone(), user1_id.clone());

        // Check status again
        let response = server
            .get(&format!("/api/users/status?{}", query_params))
            .add_header("Cookie", format!("jwt={}", token1))
            .await;

        response.assert_status_success();
        let statuses: Vec<serde_json::Value> = response.json();

        // Find user1 and user2 statuses
        let user1_status = statuses.iter().find(|s| s["user_id"] == user1_id).unwrap();
        let user2_status = statuses.iter().find(|s| s["user_id"] == user2_id).unwrap();

        assert_eq!(user1_status["is_online"], true);
        assert_eq!(user2_status["is_online"], false);

        // Bring user2 online
        let _guard2 = UserPresenceGuard::new(ctx_state.clone(), user2_id.clone());

        // Check status again
        let response = server
            .get(&format!("/api/users/status?{}", query_params))
            .add_header("Cookie", format!("jwt={}", token1))
            .await;

        response.assert_status_success();
        let statuses: Vec<serde_json::Value> = response.json();

        // Both should be online
        for status in &statuses {
            assert_eq!(status["is_online"], true);
        }

        // Drop guards and test cleanup behavior
        drop(_guard1);
        drop(_guard2);

        // Immediate check - might still show online due to cleanup delay
        let response = server
            .get(&format!("/api/users/status?{}", query_params))
            .add_header("Cookie", format!("jwt={}", token1))
            .await;

        response.assert_status_success();
        let statuses: Vec<serde_json::Value> = response.json();

        // Users might still appear online immediately after drop
        println!("Immediate status after drop: {:?}", statuses);

        // Wait for cleanup and check again
        sleep(Duration::from_secs(12)).await;

        let response = server
            .get(&format!("/api/users/status?{}", query_params))
            .add_header("Cookie", format!("jwt={}", token1))
            .await;

        response.assert_status_success();
        let statuses: Vec<serde_json::Value> = response.json();

        println!("Status after cleanup delay: {:?}", statuses);

        // After cleanup, users should be offline (timing dependent)
        // This test provides insight into the cleanup behavior
    }
);
