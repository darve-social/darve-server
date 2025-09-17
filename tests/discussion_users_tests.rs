mod helpers;

use axum_test::multipart::MultipartForm;
use darve_server::entities::community::community_entity;
use darve_server::entities::community::discussion_entity::Discussion;
use darve_server::entities::community::post_entity::Post;
use darve_server::interfaces::repositories::discussion_user::DiscussionUserRepositoryInterface;
use darve_server::middleware::utils::db_utils::Pagination;
use darve_server::models::view::discussion_user::DiscussionUserView;
use darve_server::services::discussion_service::CreateDiscussion;

use community_entity::CommunityDbService;
use serde_json::json;

use crate::helpers::create_fake_login_test_user;
use crate::helpers::post_helpers::{create_fake_post, create_post};

test_with_server!(on_create_private_discussion, |server, ctx_state, config| {
    let (server, user1, _, _) = create_fake_login_test_user(&server).await;
    let (server, user2, _, _) = create_fake_login_test_user(&server).await;
    let (server, user3, _, _) = create_fake_login_test_user(&server).await;
    let (server, user4, _, _) = create_fake_login_test_user(&server).await;
    let (server, user5, _, _) = create_fake_login_test_user(&server).await;

    let comm_id = CommunityDbService::get_profile_community_id(user5.id.as_ref().unwrap());
    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.to_raw(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![
                user1.id.as_ref().unwrap().to_raw(),
                user2.id.as_ref().unwrap().to_raw(),
                user3.id.as_ref().unwrap().to_raw(),
                user4.id.as_ref().unwrap().to_raw(),
            ]
            .into(),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_ok();

    let disc_user1 = ctx_state
        .db
        .discussion_users
        .get_by_user::<DiscussionUserView>(
            user1.id.as_ref().unwrap().id.to_raw().as_str(),
            Pagination {
                order_by: None,
                order_dir: None,
                count: 10,
                start: 0,
            },
        )
        .await
        .unwrap();

    assert_eq!(disc_user1.len(), 1);
    assert_eq!(
        disc_user1[0].discussion.title,
        Some("The Discussion".to_string())
    );
    assert_eq!(disc_user1[0].nr_unread, 0);
    assert!(disc_user1[0].latest_post.is_none());

    let disc_user2 = ctx_state
        .db
        .discussion_users
        .get_by_user::<DiscussionUserView>(
            user2.id.as_ref().unwrap().id.to_raw().as_str(),
            Pagination {
                order_by: None,
                order_dir: None,
                count: 10,
                start: 0,
            },
        )
        .await
        .unwrap();

    assert_eq!(disc_user2.len(), 1);
    assert_eq!(
        disc_user2[0].discussion.title,
        Some("The Discussion".to_string())
    );
    assert_eq!(disc_user2[0].nr_unread, 0);
    assert!(disc_user2[0].latest_post.is_none());

    let disc_user3 = ctx_state
        .db
        .discussion_users
        .get_by_user::<DiscussionUserView>(
            user3.id.as_ref().unwrap().id.to_raw().as_str(),
            Pagination {
                order_by: None,
                order_dir: None,
                count: 10,
                start: 0,
            },
        )
        .await
        .unwrap();

    assert_eq!(disc_user3.len(), 1);
    assert_eq!(
        disc_user3[0].discussion.title,
        Some("The Discussion".to_string())
    );
    assert_eq!(disc_user3[0].nr_unread, 0);
    assert!(disc_user3[0].latest_post.is_none());
    let disc_user4 = ctx_state
        .db
        .discussion_users
        .get_by_user::<DiscussionUserView>(
            user4.id.as_ref().unwrap().id.to_raw().as_str(),
            Pagination {
                order_by: None,
                order_dir: None,
                count: 10,
                start: 0,
            },
        )
        .await
        .unwrap();

    assert_eq!(disc_user4.len(), 1);
    assert_eq!(
        disc_user4[0].discussion.title,
        Some("The Discussion".to_string())
    );
    assert_eq!(disc_user4[0].nr_unread, 0);
    assert!(disc_user4[0].latest_post.is_none());
    let disc_user5 = ctx_state
        .db
        .discussion_users
        .get_by_user::<DiscussionUserView>(
            user5.id.as_ref().unwrap().id.to_raw().as_str(),
            Pagination {
                order_by: None,
                order_dir: None,
                count: 10,
                start: 0,
            },
        )
        .await
        .unwrap();

    assert_eq!(disc_user5.len(), 1);
    assert_eq!(
        disc_user5[0].discussion.title,
        Some("The Discussion".to_string())
    );
    assert_eq!(disc_user5[0].nr_unread, 0);
    assert!(disc_user5[0].latest_post.is_none());
});

test_with_server!(on_create_public_discussion, |server, ctx_state, config| {
    let (server, user3, _, _) = create_fake_login_test_user(&server).await;

    let comm_id = CommunityDbService::get_profile_community_id(user3.id.as_ref().unwrap());
    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.to_raw(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: None,
            private_discussion_users_final: false,
        })
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_ok();

    let disc_user = ctx_state
        .db
        .discussion_users
        .get_by_user::<DiscussionUserView>(
            user3.id.as_ref().unwrap().id.to_raw().as_str(),
            Pagination {
                order_by: None,
                order_dir: None,
                count: 10,
                start: 0,
            },
        )
        .await
        .unwrap();

    assert_eq!(disc_user.len(), 0);
});

test_with_server!(on_add_users_to_discussion, |server, ctx_state, config| {
    let (server, user1, _, _) = create_fake_login_test_user(&server).await;
    let (server, user2, _, _) = create_fake_login_test_user(&server).await;
    let (server, user3, _, _) = create_fake_login_test_user(&server).await;

    let comm_id = CommunityDbService::get_profile_community_id(user3.id.as_ref().unwrap());
    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.to_raw(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
            private_discussion_users_final: false,
        })
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_ok();
    let disc_id = create_response.json::<Discussion>().id;

    let post = create_fake_post(&server, &disc_id, None, None).await.id;

    let create_response = server
        .post(format!("/api/discussions/{}/chat_users", disc_id.to_raw()).as_str())
        .json(&json!({
            "user_ids": vec![user2.id.as_ref().unwrap().to_raw()]
        }))
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_ok();

    let disc_user2 = ctx_state
        .db
        .discussion_users
        .get_by_user::<DiscussionUserView>(
            user2.id.as_ref().unwrap().id.to_raw().as_str(),
            Pagination {
                order_by: None,
                order_dir: None,
                count: 10,
                start: 0,
            },
        )
        .await
        .unwrap();

    assert_eq!(disc_user2.len(), 1);
    assert_eq!(disc_user2[0].nr_unread, 0);
    assert_eq!(
        disc_user2[0].latest_post.as_ref().unwrap().id.to_raw(),
        post
    );
});

test_with_server!(
    on_remove_users_from_discussion,
    |server, ctx_state, config| {
        let (server, user1, _, _) = create_fake_login_test_user(&server).await;
        let (server, user3, _, _) = create_fake_login_test_user(&server).await;

        let comm_id = CommunityDbService::get_profile_community_id(user3.id.as_ref().unwrap());
        let create_response = server
            .post("/api/discussions")
            .json(&CreateDiscussion {
                community_id: comm_id.to_raw(),
                title: "The Discussion".to_string(),
                image_uri: None,
                chat_user_ids: vec![user1.id.as_ref().unwrap().to_raw()].into(),
                private_discussion_users_final: false,
            })
            .add_header("Accept", "application/json")
            .await;
        create_response.assert_status_ok();
        let disc_id = create_response.json::<Discussion>().id;

        let create_response = server
            .delete(format!("/api/discussions/{}/chat_users", disc_id.to_raw()).as_str())
            .json(&json!({
                "user_ids": vec![user1.id.as_ref().unwrap().to_raw()]
            }))
            .add_header("Accept", "application/json")
            .await;
        create_response.assert_status_ok();

        let disc_user1 = ctx_state
            .db
            .discussion_users
            .get_by_user::<DiscussionUserView>(
                user1.id.as_ref().unwrap().id.to_raw().as_str(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: 10,
                    start: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(disc_user1.len(), 0);
    }
);

test_with_server!(
    on_created_discussion_public_post,
    |server, ctx_state, config| {
        let (server, user1, _, _) = create_fake_login_test_user(&server).await;
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let (server, user3, _, _) = create_fake_login_test_user(&server).await;

        let comm_id = CommunityDbService::get_profile_community_id(user3.id.as_ref().unwrap());
        let create_response = server
            .post("/api/discussions")
            .json(&CreateDiscussion {
                community_id: comm_id.to_raw(),
                title: "The Discussion".to_string(),
                image_uri: None,
                chat_user_ids: vec![
                    user1.id.as_ref().unwrap().to_raw(),
                    user2.id.as_ref().unwrap().to_raw(),
                ]
                .into(),
                private_discussion_users_final: false,
            })
            .add_header("Accept", "application/json")
            .await;
        create_response.assert_status_ok();
        let disc_id = create_response.json::<Discussion>().id;
        let post = create_fake_post(&server, &disc_id, None, None).await.id;

        let disc_user2 = ctx_state
            .db
            .discussion_users
            .get_by_user::<DiscussionUserView>(
                user2.id.as_ref().unwrap().id.to_raw().as_str(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: 10,
                    start: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(disc_user2.len(), 1);
        assert_eq!(disc_user2[0].nr_unread, 1);
        assert_eq!(
            disc_user2[0].latest_post.as_ref().unwrap().id.to_raw(),
            post
        );
        let disc_user1 = ctx_state
            .db
            .discussion_users
            .get_by_user::<DiscussionUserView>(
                user1.id.as_ref().unwrap().id.to_raw().as_str(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: 10,
                    start: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(disc_user1.len(), 1);
        assert_eq!(disc_user1[0].nr_unread, 1);
        assert_eq!(
            disc_user1[0].latest_post.as_ref().unwrap().id.to_raw(),
            post
        );
        let disc_user3 = ctx_state
            .db
            .discussion_users
            .get_by_user::<DiscussionUserView>(
                user3.id.as_ref().unwrap().id.to_raw().as_str(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: 10,
                    start: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(disc_user3.len(), 1);
        assert_eq!(disc_user3[0].nr_unread, 0);
        assert_eq!(
            disc_user3[0].latest_post.as_ref().unwrap().id.to_raw(),
            post
        );
    }
);

test_with_server!(
    on_add_users_to_discussion_private_post,
    |server, ctx_state, config| {
        let (server, user1, _, _) = create_fake_login_test_user(&server).await;
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let (server, user3, _, _) = create_fake_login_test_user(&server).await;

        let comm_id = CommunityDbService::get_profile_community_id(user3.id.as_ref().unwrap());
        let create_response = server
            .post("/api/discussions")
            .json(&CreateDiscussion {
                community_id: comm_id.to_raw(),
                title: "The Discussion".to_string(),
                image_uri: None,
                chat_user_ids: vec![
                    user1.id.as_ref().unwrap().to_raw(),
                    user2.id.as_ref().unwrap().to_raw(),
                ]
                .into(),
                private_discussion_users_final: false,
            })
            .add_header("Accept", "application/json")
            .await;
        create_response.assert_status_ok();
        let disc_id = create_response.json::<Discussion>().id;
        let post = create_fake_post(&server, &disc_id, None, None).await.id;

        let data = MultipartForm::new()
            .add_text("title", "Test discussion users: create ptivate post")
            .add_text("content", "content")
            .add_text("users", user1.id.as_ref().unwrap().to_raw());

        let private_post = create_post(server, &disc_id, data).await.json::<Post>();

        let disc_user2 = ctx_state
            .db
            .discussion_users
            .get_by_user::<DiscussionUserView>(
                user2.id.as_ref().unwrap().id.to_raw().as_str(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: 10,
                    start: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(disc_user2.len(), 1);
        assert_eq!(disc_user2[0].nr_unread, 1);
        assert_eq!(
            disc_user2[0].latest_post.as_ref().unwrap().id.to_raw(),
            post
        );
        let disc_user1 = ctx_state
            .db
            .discussion_users
            .get_by_user::<DiscussionUserView>(
                user1.id.as_ref().unwrap().id.to_raw().as_str(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: 10,
                    start: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(disc_user1.len(), 1);
        assert_eq!(disc_user1[0].nr_unread, 2);
        assert_eq!(
            disc_user1[0].latest_post.as_ref().unwrap().id,
            *private_post.id.as_ref().unwrap()
        );
        let disc_user3 = ctx_state
            .db
            .discussion_users
            .get_by_user::<DiscussionUserView>(
                user3.id.as_ref().unwrap().id.to_raw().as_str(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: 10,
                    start: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(disc_user3.len(), 1);
        assert_eq!(disc_user3[0].nr_unread, 0);
        assert_eq!(
            disc_user3[0].latest_post.as_ref().unwrap().id,
            *private_post.id.as_ref().unwrap()
        );
    }
);

test_with_server!(
    on_remove_users_from_discussion_private_post,
    |server, ctx_state, config| {
        let (server, user1, _, _) = create_fake_login_test_user(&server).await;
        let (server, user2, _, _) = create_fake_login_test_user(&server).await;
        let (server, user3, _, _) = create_fake_login_test_user(&server).await;

        let comm_id = CommunityDbService::get_profile_community_id(user3.id.as_ref().unwrap());
        let create_response = server
            .post("/api/discussions")
            .json(&CreateDiscussion {
                community_id: comm_id.to_raw(),
                title: "The Discussion".to_string(),
                image_uri: None,
                chat_user_ids: vec![
                    user1.id.as_ref().unwrap().to_raw(),
                    user2.id.as_ref().unwrap().to_raw(),
                ]
                .into(),
                private_discussion_users_final: false,
            })
            .add_header("Accept", "application/json")
            .await;
        create_response.assert_status_ok();
        let disc_id = create_response.json::<Discussion>().id;
        let post = create_fake_post(&server, &disc_id, None, None).await.id;

        let data = MultipartForm::new()
            .add_text("title", "Test discussion users: create ptivate post")
            .add_text("content", "content")
            .add_text("users", user1.id.as_ref().unwrap().to_raw())
            .add_text("users", user2.id.as_ref().unwrap().to_raw());

        let private_post = create_post(server, &disc_id, data).await.json::<Post>();

        server
            .post(&format!(
                "/api/posts/{}/remove_users",
                private_post.id.as_ref().unwrap().to_raw()
            ))
            .json(&json!({ "user_ids": [user2.id.as_ref().unwrap().to_raw()]}))
            .await
            .assert_status_success();

        let disc_user2 = ctx_state
            .db
            .discussion_users
            .get_by_user::<DiscussionUserView>(
                user2.id.as_ref().unwrap().id.to_raw().as_str(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: 10,
                    start: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(disc_user2.len(), 1);
        assert_eq!(disc_user2[0].nr_unread, 1);
        assert_eq!(
            disc_user2[0].latest_post.as_ref().unwrap().id.to_raw(),
            post
        );
        let disc_user1 = ctx_state
            .db
            .discussion_users
            .get_by_user::<DiscussionUserView>(
                user1.id.as_ref().unwrap().id.to_raw().as_str(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: 10,
                    start: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(disc_user1.len(), 1);
        assert_eq!(disc_user1[0].nr_unread, 2);
        assert_eq!(
            disc_user1[0].latest_post.as_ref().unwrap().id,
            *private_post.id.as_ref().unwrap()
        );
        let disc_user3 = ctx_state
            .db
            .discussion_users
            .get_by_user::<DiscussionUserView>(
                user3.id.as_ref().unwrap().id.to_raw().as_str(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: 10,
                    start: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(disc_user3.len(), 1);
        assert_eq!(disc_user3[0].nr_unread, 0);
        assert_eq!(
            disc_user3[0].latest_post.as_ref().unwrap().id,
            *private_post.id.as_ref().unwrap()
        );
    }
);

test_with_server!(
    on_remove_users_from_seen_discussion_private_post,
    |server, ctx_state, config| {
        let (server, user1, _, _) = create_fake_login_test_user(&server).await;
        let (server, user2, _, token2) = create_fake_login_test_user(&server).await;
        let (server, user3, _, _) = create_fake_login_test_user(&server).await;

        let comm_id = CommunityDbService::get_profile_community_id(user3.id.as_ref().unwrap());
        let create_response = server
            .post("/api/discussions")
            .json(&CreateDiscussion {
                community_id: comm_id.to_raw(),
                title: "The Discussion".to_string(),
                image_uri: None,
                chat_user_ids: vec![
                    user1.id.as_ref().unwrap().to_raw(),
                    user2.id.as_ref().unwrap().to_raw(),
                ]
                .into(),
                private_discussion_users_final: false,
            })
            .add_header("Accept", "application/json")
            .await;
        create_response.assert_status_ok();
        let disc_id = create_response.json::<Discussion>().id;
        let post = create_fake_post(&server, &disc_id, None, None).await.id;

        let data = MultipartForm::new()
            .add_text("title", "Test discussion users: create ptivate post")
            .add_text("content", "content")
            .add_text("users", user1.id.as_ref().unwrap().to_raw())
            .add_text("users", user2.id.as_ref().unwrap().to_raw());

        let private_post = create_post(server, &disc_id, data).await.json::<Post>();

        server
            .post(&format!(
                "/api/posts/{}/read",
                private_post.id.as_ref().unwrap().to_raw()
            ))
            .json(&json!({ "user_ids": [user2.id.as_ref().unwrap().to_raw()]}))
            .add_header("Cookie", format!("jwt={}", token2))
            .await
            .assert_status_success();

        server
            .post(&format!(
                "/api/posts/{}/remove_users",
                private_post.id.as_ref().unwrap().to_raw()
            ))
            .json(&json!({ "user_ids": [user2.id.as_ref().unwrap().to_raw()]}))
            .await
            .assert_status_success();

        let disc_user2 = ctx_state
            .db
            .discussion_users
            .get_by_user::<DiscussionUserView>(
                user2.id.as_ref().unwrap().id.to_raw().as_str(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: 10,
                    start: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(disc_user2.len(), 1);
        assert_eq!(disc_user2[0].nr_unread, 1);
        assert_eq!(
            disc_user2[0].latest_post.as_ref().unwrap().id.to_raw(),
            post
        );
        let disc_user1 = ctx_state
            .db
            .discussion_users
            .get_by_user::<DiscussionUserView>(
                user1.id.as_ref().unwrap().id.to_raw().as_str(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: 10,
                    start: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(disc_user1.len(), 1);
        assert_eq!(disc_user1[0].nr_unread, 2);
        assert_eq!(
            disc_user1[0].latest_post.as_ref().unwrap().id,
            *private_post.id.as_ref().unwrap()
        );
        let disc_user3 = ctx_state
            .db
            .discussion_users
            .get_by_user::<DiscussionUserView>(
                user3.id.as_ref().unwrap().id.to_raw().as_str(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: 10,
                    start: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(disc_user3.len(), 1);
        assert_eq!(disc_user3[0].nr_unread, 0);
        assert_eq!(
            disc_user3[0].latest_post.as_ref().unwrap().id,
            *private_post.id.as_ref().unwrap()
        );
    }
);

test_with_server!(on_seen_post, |server, ctx_state, config| {
    let (server, user1, _, _) = create_fake_login_test_user(&server).await;
    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;
    let (server, user3, _, _) = create_fake_login_test_user(&server).await;

    let comm_id = CommunityDbService::get_profile_community_id(user3.id.as_ref().unwrap());
    let create_response = server
        .post("/api/discussions")
        .json(&CreateDiscussion {
            community_id: comm_id.to_raw(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![
                user1.id.as_ref().unwrap().to_raw(),
                user2.id.as_ref().unwrap().to_raw(),
            ]
            .into(),
            private_discussion_users_final: false,
        })
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_ok();
    let disc_id = create_response.json::<Discussion>().id;
    let post = create_fake_post(&server, &disc_id, None, None).await.id;

    let data = MultipartForm::new()
        .add_text("title", "Test discussion users: create ptivate post")
        .add_text("content", "content")
        .add_text("users", user1.id.as_ref().unwrap().to_raw())
        .add_text("users", user2.id.as_ref().unwrap().to_raw());

    let private_post = create_post(server, &disc_id, data).await.json::<Post>();

    server
        .post(&format!(
            "/api/posts/{}/read",
            private_post.id.as_ref().unwrap().to_raw()
        ))
        .json(&json!({ "user_ids": [user2.id.as_ref().unwrap().to_raw()]}))
        .add_header("Cookie", format!("jwt={}", token2))
        .await
        .assert_status_success();

    let disc_user2 = ctx_state
        .db
        .discussion_users
        .get_by_user::<DiscussionUserView>(
            user2.id.as_ref().unwrap().id.to_raw().as_str(),
            Pagination {
                order_by: None,
                order_dir: None,
                count: 10,
                start: 0,
            },
        )
        .await
        .unwrap();

    assert_eq!(disc_user2.len(), 1);
    assert_eq!(disc_user2[0].nr_unread, 1);
    assert_eq!(
        disc_user2[0].latest_post.as_ref().unwrap().id,
        *private_post.id.as_ref().unwrap()
    );
    let disc_user1 = ctx_state
        .db
        .discussion_users
        .get_by_user::<DiscussionUserView>(
            user1.id.as_ref().unwrap().id.to_raw().as_str(),
            Pagination {
                order_by: None,
                order_dir: None,
                count: 10,
                start: 0,
            },
        )
        .await
        .unwrap();

    assert_eq!(disc_user1.len(), 1);
    assert_eq!(disc_user1[0].nr_unread, 2);
    assert_eq!(
        disc_user1[0].latest_post.as_ref().unwrap().id,
        *private_post.id.as_ref().unwrap()
    );
    let disc_user3 = ctx_state
        .db
        .discussion_users
        .get_by_user::<DiscussionUserView>(
            user3.id.as_ref().unwrap().id.to_raw().as_str(),
            Pagination {
                order_by: None,
                order_dir: None,
                count: 10,
                start: 0,
            },
        )
        .await
        .unwrap();

    assert_eq!(disc_user3.len(), 1);
    assert_eq!(disc_user3[0].nr_unread, 0);
    assert_eq!(
        disc_user3[0].latest_post.as_ref().unwrap().id,
        *private_post.id.as_ref().unwrap()
    );
});

test_with_server!(get_latest_post, |server, ctx_state, config| {});
