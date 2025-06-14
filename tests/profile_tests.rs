mod helpers;
use axum_test::multipart::MultipartForm;
use darve_server::{
    entities::{
        user_auth::local_user_entity::LocalUserDbService, verification_code::VerificationCodeFor,
    },
    interfaces::repositories::verification_code::VerificationCodeRepositoryInterface,
    middleware::{self, utils::db_utils::UsernameIdent},
    routes::{
        community::{
            discussion_routes,
            profile_routes::{self},
        },
        user_auth::login_routes,
    },
};
use discussion_routes::get_discussion_view;
use helpers::{
    create_fake_login_test_user, create_login_test_user, create_test_server, post_helpers,
    user_helpers::{self, get_user, update_current_user},
};
use login_routes::LoginInput;
use middleware::ctx::Ctx;
use middleware::error::AppError;
use middleware::utils::extractor_utils::DiscussionParams;
use middleware::utils::request_utils::CreatedResponse;
use profile_routes::{ProfileChat, ProfileChatList, SearchInput};
use serde_json::json;
use uuid::Uuid;

use crate::helpers::fake_username_min_len;

#[tokio::test]
async fn search_users() {
    let (server, _) = create_test_server().await;
    let username1 = "its_user_one".to_string();
    let username2 = "its_user_two".to_string();
    let username3 = "herodus".to_string();
    let (server, _) = create_login_test_user(&server, username1.clone()).await;
    let (server, _) = create_login_test_user(&server, username2.clone()).await;
    let (server, _) = create_login_test_user(&server, username3.clone()).await;

    let request = server
        .post("/api/accounts/edit")
        .multipart(
            MultipartForm::new()
                .add_text("username", "username_new")
                .add_text("full_name", "Full Name Userset")
                .add_text("email", "ome@email.com"),
        )
        .add_header("Accept", "application/json")
        .await;
    request.assert_status_success();

    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "rset".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 1);

    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "user".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 3);

    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "Userse".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 1);

    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "one".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 1);

    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "unknown".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 0);

    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "its".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 2);

    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "hero".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 0);

    let (server, _) = create_login_test_user(&server, String::from("abcdtest")).await;
    let (server, _) = create_login_test_user(&server, String::from("abcdtest1")).await;
    let (server, _) = create_login_test_user(&server, String::from("abcdtest2")).await;
    let (server, _) = create_login_test_user(&server, String::from("abcdtest3")).await;
    let (server, _) = create_login_test_user(&server, String::from("abcdtest4")).await;
    let res = user_helpers::create_user(
        &server,
        &SearchInput {
            query: "tes".to_string(),
        },
    )
    .await;
    assert_eq!(res.items.len(), 5);
}

#[tokio::test]
async fn get_user_chat() {
    let (server, ctx_state) = create_test_server().await;
    let (server, user1, user1_pwd) = create_fake_login_test_user(&server).await;
    let user_ident1 = user1.id.as_ref().unwrap().to_raw();

    let (server, user2, _) = create_fake_login_test_user(&server).await;
    let user_ident2: String = user2.id.as_ref().unwrap().to_raw();
    let username1 = user1.username.to_string();
    let username3 = fake_username_min_len(9);

    // logged in as username2
    // get user chats
    let create_response = server
        .get("/api/user_chat/list")
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<ProfileChatList>();
    assert_eq!(created.discussions.len(), 0);
    assert_eq!(created.user_id.to_raw(), user_ident2);

    // create chat with user_ident1
    let create_response = server
        .get(format!("/api/user_chat/with/{}", user_ident1.as_str()).as_str())
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_success();
    let created = &create_response.json::<ProfileChat>();
    let chat_disc_id = created.discussion.id.clone().unwrap();

    // send message
    let post_name = "post title Name 1".to_string();
    let create_post = server
        .post(format!("/api/discussion/{chat_disc_id}/post").as_str())
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "contentttt")
                .add_text("topic_id", ""),
        )
        .add_header("Accept", "application/json")
        .await;
    let created = create_post.json::<CreatedResponse>();
    create_post.assert_status_success();
    assert_eq!(created.id.len() > 0, true);

    // check chat exists in list
    let create_response = server
        .get("/api/user_chat/list")
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<ProfileChatList>();

    assert_eq!(created.discussions.len(), 1);
    assert_eq!(created.user_id.to_raw(), user_ident2);
    let list_disc_id = created.discussions.get(0).unwrap().id.clone().unwrap();
    assert_eq!(list_disc_id, chat_disc_id);
    let disc = created.discussions.first();
    let latest_post = &disc.as_ref().unwrap().latest_post.as_ref().unwrap();
    assert!(latest_post.r_created.is_some());
    // login username1
    server.get("/logout").await;
    let login_response = server
        .post("/api/login")
        .json(&LoginInput {
            username: username1.clone(),
            password: user1_pwd.clone(),
            next: None,
        })
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_success();

    // logged in as username1
    // check user1 also has chat with user2
    let create_response = server
        .get("/api/user_chat/list")
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_success();
    let created = &create_response.json::<ProfileChatList>();
    assert_eq!(created.discussions.len(), 1);

    assert_eq!(created.user_id.to_raw(), user_ident1);
    let list_disc_id = created.discussions.get(0).unwrap().id.clone().unwrap();
    assert_eq!(list_disc_id, chat_disc_id);

    // respond to chat
    let create_response = server
        .get(format!("/api/user_chat/with/{}", user_ident2.as_str()).as_str())
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_success();
    let created = &create_response.json::<ProfileChat>();
    let chat_disc_id_usr1 = created.discussion.id.clone().unwrap();
    assert_eq!(chat_disc_id_usr1, chat_disc_id.clone());

    let ctx = Ctx::new(Ok(user_ident1), Uuid::new_v4(), false);
    let discussion_posts = get_discussion_view(
        &ctx_state.db.client,
        &ctx,
        chat_disc_id.clone(),
        DiscussionParams {
            topic_id: None,
            start: None,
            count: None,
        },
    )
    .await
    .expect("discussion");
    assert_eq!(discussion_posts.posts.len(), 1);
    assert_eq!(
        discussion_posts.posts.get(0).expect("post").title,
        post_name
    );

    // create new user
    let (_, user_ident3) = create_login_test_user(&server, username3.clone()).await;
    let ctx = Ctx::new(Ok(user_ident3), Uuid::new_v4(), false);
    let d_view = get_discussion_view(
        &ctx_state.db.client,
        &ctx,
        chat_disc_id.clone(),
        DiscussionParams {
            topic_id: None,
            start: None,
            count: None,
        },
    )
    .await;
    assert_eq!(d_view.is_err(), true);
    assert_eq!(
        d_view.err().unwrap().error,
        AppError::AuthorizationFail {
            required: "Is chat participant".to_string()
        }
    );
}

#[tokio::test]
async fn get_user_chat_1() {
    let (server, _) = create_test_server().await;
    let (_, local_user_1, _) = create_fake_login_test_user(&server).await;
    let (_, local_user_2, _) = create_fake_login_test_user(&server).await;

    let create_response = server
        .get("/api/user_chat/list")
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<ProfileChatList>();
    assert_eq!(created.discussions.len(), 0);
    assert_eq!(created.user_id, local_user_2.id.clone().unwrap());

    let create_response = server
        .get(format!("/api/user_chat/with/{}", local_user_1.id.unwrap().to_raw()).as_str())
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_success();
    let created = &create_response.json::<ProfileChat>();
    let chat_disc_id = created.discussion.id.clone().unwrap();

    let _ = post_helpers::create_fake_post(&server, &chat_disc_id, None, None).await;
    // check chat exists in list
    let create_response = server
        .get("/api/user_chat/list")
        .add_header("Accept", "application/json")
        .await;

    let created = &create_response.json::<ProfileChatList>();
    assert_eq!(created.discussions.len(), 1);
    let user_2_id = local_user_2.id.clone();
    assert_eq!(created.user_id, user_2_id.unwrap());
    let list_disc_id = created.discussions.get(0).unwrap().id.clone().unwrap();
    assert_eq!(list_disc_id, chat_disc_id);

    let disc_view = created.discussions.get(0).unwrap();
    assert!(disc_view.latest_post.is_some());
    let create_by = &disc_view.latest_post.as_ref().unwrap().created_by;

    assert_eq!(create_by.username, local_user_2.username);
    assert_eq!(create_by.full_name, local_user_2.full_name);
    assert_eq!(create_by.image_uri, local_user_2.image_uri)
}

#[tokio::test]
async fn email_verification_and_confirmation() {
    let (server, ctx_state) = create_test_server().await;

    let (server, user, _) = create_fake_login_test_user(&server).await;
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
    let ctx = Ctx::new(Ok(user_id.to_raw()), uuid::Uuid::new_v4(), false);
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
        .get_by_user(&user_id.to_raw(), VerificationCodeFor::EmailVerification)
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
        .get_by_user(&user_id.to_raw(), VerificationCodeFor::EmailVerification)
        .await;

    assert!(code.is_err());
}

#[tokio::test]
async fn email_confirmation_with_invalid_code() {
    let (server, _) = create_test_server().await;

    let (server, _, _pwd) = create_fake_login_test_user(&server).await;
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

#[tokio::test]
async fn update_user_avatar() {
    let (server, _) = create_test_server().await;
    let (_, local_user, _pwd) = create_fake_login_test_user(&server).await;

    let create_response = update_current_user(&server).await;
    create_response.assert_status_success();

    let get_response = get_user(&server, local_user.id.unwrap().to_raw().as_str()).await;
    get_response.assert_status_success();

    let user = get_response.json::<profile_routes::ProfilePage>();
    assert!(user
        .profile_view
        .unwrap()
        .image_uri
        .as_ref()
        .unwrap()
        .contains("profile_image.jpg"));
}

#[tokio::test]
async fn set_user_password() {
    let (server, state) = create_test_server().await;
    let new_password = "setPassword123";

    // Create and login user
    let (server, local_user, _old_password) = create_fake_login_test_user(&server).await;

    // Set password using POST endpoint (no old password required)
    let response = server
        .post("/api/users/current/password")
        .json(&serde_json::json!({
            "password": new_password
        }))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_failure();
    assert!(response.text().contains("User has already set a password"));

    let _ = state
        .db
        .client
        .query("DELETE authentication WHERE local_user=$user")
        .bind(("user", local_user.id.unwrap()))
        .await;

    let response = server
        .post("/api/users/current/password")
        .json(&serde_json::json!({
            "password": new_password
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
            "username": local_user.username,
            "password": new_password
        }))
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_success();

    // Test validation: password too short should fail
    let response = server
        .post("/api/users/current/password")
        .json(&serde_json::json!({
            "password": "12345" // Less than 6 characters
        }))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_failure();
}

#[tokio::test]
async fn update_user_password() {
    let (server, _) = create_test_server().await;
    let new_password = "newPassword456";

    // Create and login user
    let (server, local_user, password) = create_fake_login_test_user(&server).await;

    // Try to update password with wrong old password
    let response = server
        .patch("/api/users/current/password")
        .json(&serde_json::json!({
            "old_password": "wrongPassword",
            "new_password": new_password
        }))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_failure();

    // Update password with correct old password
    let response = server
        .patch("/api/users/current/password")
        .json(&serde_json::json!({
            "old_password": password,
            "new_password": new_password
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
            "username": local_user.username,
            "password": password
        }))
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_failure();

    // Login with new password (should succeed)
    let login_response = server
        .post("/api/login")
        .json(&serde_json::json!({
            "username": local_user.username,
            "password": new_password
        }))
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_success();
}
