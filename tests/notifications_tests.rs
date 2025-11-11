mod helpers;

use crate::helpers::create_fake_login_test_user;
use darve_server::{
    entities::{
        community::{
            community_entity::CommunityDbService,
            discussion_entity::{Discussion, DiscussionDbService},
        },
        user_notification::UserNotificationEvent,
    },
    models::view::notification::UserNotificationView,
    services::discussion_service::CreateDiscussion,
};

use fake::{faker, Fake};
use helpers::post_helpers::create_fake_post;
use serde_json::json;

test_with_server!(on_create_private_task, |server, ctx_state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let disc_id = DiscussionDbService::get_profile_discussion_id(user1.id.as_ref().unwrap());
    let post = create_fake_post(server, &disc_id, None, None).await;

    let endow_user_response = server
        .get(&format!("/test/api/deposit/{}/{}", user1.username, 1000))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    endow_user_response.assert_status_success();

    let task_request = server
        .post(format!("/api/posts/{}/tasks", post.id).as_str())
        .json(&json!({
            "offer_amount": Some(100),
            "participant": Some(user0.id.as_ref().unwrap().to_raw()),
            "content":faker::lorem::en::Sentence(7..20).fake::<String>()
        }))
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await;
    task_request.assert_status_success();
    let notifications = server
        .get("/api/notifications")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await
        .json::<Vec<UserNotificationView>>();
    assert!(notifications
        .iter()
        .find(|n| n.event == UserNotificationEvent::UserTaskRequestCreated)
        .is_none());
    assert!(notifications
        .iter()
        .find(|n| n.event == UserNotificationEvent::UserTaskRequestReceived)
        .is_some());
});

test_with_server!(
    on_create_private_task_in_disc,
    |server, ctx_state, config| {
        let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
        let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let endow_user_response = server
            .get(&format!("/test/api/deposit/{}/{}", user2.username, 1000))
            .add_header("Accept", "application/json")
            .await;
        endow_user_response.assert_status_success();

        let disc_id = server
            .post("/api/discussions")
            .json(&CreateDiscussion {
                community_id: CommunityDbService::get_profile_community_id(
                    user2.id.as_ref().unwrap(),
                )
                .to_raw(),
                title: "Create private discussion".to_string(),
                image_uri: None,
                chat_user_ids: Some(
                    [
                        user1.id.as_ref().unwrap().to_raw(),
                        user0.id.as_ref().unwrap().to_raw(),
                    ]
                    .to_vec(),
                ),
                private_discussion_users_final: false,
            })
            .await
            .json::<Discussion>()
            .id;

        let task_request = server
            .post(format!("/api/discussions/{}/tasks", disc_id.to_raw()).as_str())
            .json(&json!({
                "offer_amount": Some(100),
                "participant": Some(user0.id.as_ref().unwrap().to_raw()),
                "content":faker::lorem::en::Sentence(7..20).fake::<String>()
            }))
            .add_header("Accept", "application/json")
            .await;
        task_request.assert_status_success();
        let notifications = server
            .get("/api/notifications")
            .add_header("Cookie", format!("jwt={}", token0))
            .add_header("Accept", "application/json")
            .await
            .json::<Vec<UserNotificationView>>();
        assert!(notifications
            .iter()
            .find(|n| n.event == UserNotificationEvent::UserTaskRequestCreated)
            .is_none());
        assert!(notifications
            .iter()
            .find(|n| n.event == UserNotificationEvent::UserTaskRequestReceived)
            .is_some());

        let notifications = server
            .get("/api/notifications")
            .add_header("Cookie", format!("jwt={}", token1))
            .add_header("Accept", "application/json")
            .await
            .json::<Vec<UserNotificationView>>();
        assert!(notifications
            .iter()
            .find(|n| n.event == UserNotificationEvent::UserTaskRequestCreated)
            .is_some());
        assert!(notifications
            .iter()
            .find(|n| n.event == UserNotificationEvent::UserTaskRequestReceived)
            .is_none());
    }
);

test_with_server!(on_create_private_discussion, |server, ctx_state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;

    server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: CommunityDbService::get_profile_community_id(user2.id.as_ref().unwrap())
                .to_raw(),
            title: "Create private discussion".to_string(),
            image_uri: None,
            chat_user_ids: Some(
                [
                    user1.id.as_ref().unwrap().to_raw(),
                    user0.id.as_ref().unwrap().to_raw(),
                ]
                .to_vec(),
            ),
            private_discussion_users_final: false,
        })
        .await
        .json::<Discussion>()
        .id;

    let notifications = server
        .get("/api/notifications")
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await
        .json::<Vec<UserNotificationView>>();
    assert!(notifications
        .iter()
        .find(|n| n.event == UserNotificationEvent::CreatedDiscussion)
        .is_some());

    let notifications = server
        .get("/api/notifications")
        .add_header("Cookie", format!("jwt={}", token1))
        .add_header("Accept", "application/json")
        .await
        .json::<Vec<UserNotificationView>>();
    assert!(notifications
        .iter()
        .find(|n| n.event == UserNotificationEvent::CreatedDiscussion)
        .is_some());
    let notifications = server
        .get("/api/notifications")
        .add_header("Cookie", format!("jwt={}", token2))
        .add_header("Accept", "application/json")
        .await
        .json::<Vec<UserNotificationView>>();
    assert!(notifications
        .iter()
        .find(|n| n.event == UserNotificationEvent::CreatedDiscussion)
        .is_none());
});

test_with_server!(filter_by_types_notification, |server, ctx_state, config| {
    let (server, user0, _, token0) = create_fake_login_test_user(&server).await;
    let (server, _user2, _, _token2) = create_fake_login_test_user(&server).await;

    server
        .post(format!("/api/following/{}", user0.id.as_ref().unwrap().to_raw()).as_str())
        .await
        .assert_status_success();

    let notifications = server
        .get("/api/notifications")
        .add_query_param(
            "filter_by_types",
            UserNotificationEvent::UserFollowAdded.as_str(),
        )
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await
        .json::<Vec<UserNotificationView>>();

    assert_eq!(notifications.len(), 1);
    let notifications = server
        .get("/api/notifications")
        .add_query_param(
            "filter_by_types",
            UserNotificationEvent::UserFollowAdded.as_str(),
        )
        .add_query_param(
            "filter_by_types",
            UserNotificationEvent::UserLikePost.as_str(),
        )
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await
        .json::<Vec<UserNotificationView>>();

    assert_eq!(notifications.len(), 1);
    let notifications = server
        .get("/api/notifications")
        .add_query_param(
            "filter_by_types",
            UserNotificationEvent::UserLikePost.as_str(),
        )
        .add_header("Cookie", format!("jwt={}", token0))
        .add_header("Accept", "application/json")
        .await
        .json::<Vec<UserNotificationView>>();

    assert_eq!(notifications.len(), 0);
});
