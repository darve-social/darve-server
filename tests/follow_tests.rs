mod helpers;
use crate::helpers::{create_login_test_user, create_test_server};
use axum_test::multipart::MultipartForm;
use darve_server::{
    entities::{
        community::post_stream_entity::PostStreamDbService,
        user_auth::{follow_entity, user_notification_entity},
    },
    events_handler::application_event_handler,
    middleware,
    routes::{
        community::profile_routes::{self, get_profile_community},
        user_auth::{follow_routes, login_routes},
    },
};
use fake::{faker, Fake};
use follow_entity::FollowDbService;
use follow_routes::UserListView;
use helpers::post_helpers::create_fake_post;
use login_routes::LoginInput;
use middleware::ctx::Ctx;
use middleware::utils::request_utils::CreatedResponse;
use middleware::utils::string_utils::get_string_thing;
use profile_routes::{FollowingStreamView, ProfileDiscussionView, ProfilePage};
use user_notification_entity::UserNotification;
use uuid::Uuid;

#[tokio::test]
async fn get_user_followers() {
    let (server, ctx_state) = create_test_server().await;
    let username1 = "usnnnn".to_string();
    let username2 = "usnnnn2".to_string();
    let username3 = "usnnnn3".to_string();
    let (server, user_ident1) = create_login_test_user(&server, username1.clone()).await;
    let (server, user_ident2) = create_login_test_user(&server, username2.clone()).await;
    let user1_id = get_string_thing(user_ident1.clone()).expect("user1");
    let user2_id = get_string_thing(user_ident2.clone()).expect("user2");

    let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);
    let follow_db_service = FollowDbService {
        ctx: &ctx,
        db: &ctx_state._db,
    };
    let followers_nr = follow_db_service
        .user_followers_number(user1_id.clone())
        .await
        .expect("user 1 followers nr");
    assert_eq!(0, followers_nr);

    let is_following = follow_db_service
        .is_following(user1_id.clone(), user2_id.clone())
        .await
        .expect("is_following");
    assert_eq!(is_following, false);

    let profile1_response = server
        .get(format!("/u/{}", username1.clone()).as_str())
        .add_header("Accept", "application/json")
        .add_header("Accept", "application/json")
        .await;
    let created = profile1_response.json::<ProfilePage>();
    assert_eq!(created.profile_view.unwrap().followers_nr, 0);

    // logged in as username2
    // follow user_ident1
    let create_response = server
        .post(format!("/api/follow/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .json("")
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<CreatedResponse>();
    assert_eq!(created.success, true);

    // refollow error
    let create_response = server
        .post(format!("/api/follow/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .json("")
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_failure();

    let followers_nr = follow_db_service
        .user_followers_number(user1_id.clone())
        .await
        .expect("user 1 followers nr");
    assert_eq!(1, followers_nr);

    let is_following = follow_db_service
        .is_following(user1_id.clone(), user2_id.clone())
        .await
        .expect("is_following");
    assert_eq!(is_following, false);
    let is_following = follow_db_service
        .is_following(user2_id.clone(), user1_id.clone())
        .await
        .expect("is_following");
    assert_eq!(is_following, true);

    let profile1_response = server
        .get(format!("/u/{}", username1.clone()).as_str())
        .add_header("Accept", "application/json")
        .add_header("Accept", "application/json")
        .await;
    let created = profile1_response.json::<ProfilePage>();
    assert_eq!(created.profile_view.unwrap().followers_nr, 1);

    //login as username3
    let (server, _) = create_login_test_user(&server, username3.clone()).await;

    // follow u1
    let create_response = server
        .post(format!("/api/follow/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .json("")
        .await;
    let created = &create_response.json::<CreatedResponse>();
    assert_eq!(created.success, true);

    // refollow error
    let create_response = server
        .post(format!("/api/follow/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .json("")
        .await;
    create_response.assert_status_failure();

    // check nr of followers
    let followers_nr = follow_db_service
        .user_followers_number(user1_id.clone())
        .await
        .expect("user 1 followers nr");
    assert_eq!(2, followers_nr);

    // check nr of followers
    let profile1_response = server
        .get(format!("/u/{}", username1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    let created = profile1_response.json::<ProfilePage>();
    assert_eq!(created.profile_view.unwrap().followers_nr, 2);

    // check if follows user1
    let create_response = server
        .get(format!("/api/user/follows/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<CreatedResponse>();
    assert_eq!(created.success, true);

    // check followers for user1
    let create_response = server
        .get(format!("/api/user/{}/followers", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<UserListView>();
    assert_eq!(created.items.len(), 2);
    let f_usernames: Vec<String> = created.items.iter().map(|fu| fu.username.clone()).collect();
    assert_eq!(f_usernames.contains(&username2.clone()), true);
    assert_eq!(f_usernames.contains(&username3.clone()), true);
    assert_eq!(f_usernames.contains(&username1.clone()), false);

    // user1 follows 0
    let create_response = server
        .get(format!("/api/user/{}/following", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<UserListView>();
    assert_eq!(created.items.len(), 0);

    // user3 get followers stream
    let create_response = server
        .get("/u/following/posts")
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<FollowingStreamView>();
    assert_eq!(created.post_list.len(), 0);

    // login user1
    server.get("/logout").await;
    let login_response = server
        .post("/api/login")
        .json(&LoginInput {
            username: username1.clone(),
            password: "some3242paSs#$".to_string(),
            next: None,
        })
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_success();

    // user1 post
    let post_name = "post title Name 1".to_string();
    let create_post = server
        .post("/api/user/post")
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

    let response = server
        .get(format!("/api/user/{}/posts", username1).as_str())
        .add_header("Accept", "application/json")
        .await;
    let parsed_response = response.json::<ProfileDiscussionView>();
    create_post.assert_status_success();
    assert_eq!(parsed_response.posts.len(), 1);
    assert_eq!(
        parsed_response.posts[0].username.clone().unwrap(),
        username1
    );

    // login user3
    server.get("/logout").await;
    let login_response = server
        .post("/api/login")
        .json(&LoginInput {
            username: username3.clone(),
            password: "some3242paSs#$".to_string(),
            next: None,
        })
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_success();

    // user3 get followers stream
    let create_response = server
        .get("/u/following/posts")
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<FollowingStreamView>();
    assert_eq!(created.post_list.len(), 1);

    // login user1
    server
        .get("/logout")
        .add_header("Accept", "application/json")
        .await;
    let login_response = server
        .post("/api/login")
        .json(&LoginInput {
            username: username1.clone(),
            password: "some3242paSs#$".to_string(),
            next: None,
        })
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_success();

    // user1 post 2
    let post_name = "post title Name 2".to_string();
    let create_post = server
        .post("/api/user/post")
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "contentttt22")
                .add_text("topic_id", ""),
        )
        .add_header("Accept", "application/json")
        .await;
    let created = create_post.json::<CreatedResponse>();
    create_post.assert_status_success();
    assert_eq!(created.id.len() > 0, true);

    // login user3
    server.get("/logout").await;
    let login_response = server
        .post("/api/login")
        .json(&LoginInput {
            username: username3.clone(),
            password: "some3242paSs#$".to_string(),
            next: None,
        })
        .await;
    login_response.assert_status_success();

    // user3 get followers stream
    let create_response = server
        .get("/u/following/posts")
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<FollowingStreamView>();
    assert_eq!(created.post_list.len(), 2);

    // user3 unfollow user1
    let create_response = server
        .delete(format!("/api/follow/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<CreatedResponse>();
    assert_eq!(created.success, true);

    // check nr of user1 followers
    let profile1_response = server
        .get(format!("/u/{}", username1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    let created = profile1_response.json::<ProfilePage>();
    assert_eq!(created.profile_view.unwrap().followers_nr, 1);

    // check nr of user1 followers
    let create_response = server
        .get(format!("/api/user/{}/followers", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<UserListView>();
    assert_eq!(created.items.len(), 1);

    // check user3 unfollowed user1
    let create_response = server
        .get(format!("/api/user/follows/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<CreatedResponse>();
    assert_eq!(created.success, false);

    // login user1
    server.get("/logout").await;
    let login_response = server
        .post("/api/login")
        .json(&LoginInput {
            username: username1.clone(),
            password: "some3242paSs#$".to_string(),
            next: None,
        })
        .await;
    login_response.assert_status_success();

    // user1 post 3
    let post_name = "post title Name 3".to_string();
    let create_post = server
        .post("/api/user/post")
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "contentttt3")
                .add_text("topic_id", ""),
        )
        .add_header("Accept", "application/json")
        .await;
    let created = create_post.json::<CreatedResponse>();
    create_post.assert_status_success();
    assert_eq!(created.id.len() > 0, true);

    // login user3
    server.get("/logout").await;
    let login_response = server
        .post("/api/login")
        .json(&LoginInput {
            username: username3.clone(),
            password: "some3242paSs#$".to_string(),
            next: None,
        })
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_success();

    // user3 get followers stream
    let create_response = server
        .get("/u/following/posts")
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<FollowingStreamView>();
    assert_eq!(created.post_list.len(), 2);

    let notifications: surrealdb::Result<Vec<UserNotification>> = ctx_state
        ._db
        .select(user_notification_entity::TABLE_NAME)
        .await;
    dbg!(&notifications);
    assert_eq!(notifications.is_ok(), true);
}

#[tokio::test(flavor = "multi_thread")]
async fn add_latest_three_posts_of_follower_to_ctx_user() {
    let (server, ctx_state) = create_test_server().await;
    let task = application_event_handler(&ctx_state);

    let (_, user_ident1) =
        create_login_test_user(&server, faker::internet::en::Username().fake::<String>()).await;

    let user1_id = get_string_thing(user_ident1.clone()).expect("user1");
    let ctx = Ctx::new(Ok(user_ident1.clone()), Uuid::new_v4(), false);

    let profile_discussion = get_profile_community(&ctx_state._db, &ctx, user1_id.clone())
        .await
        .unwrap()
        .profile_discussion
        .unwrap();

    let _ = create_fake_post(&server, &profile_discussion).await;
    let post_2 = create_fake_post(&server, &profile_discussion).await;
    let post_3 = create_fake_post(&server, &profile_discussion).await;
    let post_4 = create_fake_post(&server, &profile_discussion).await;

    let (_, user_ident2) =
        create_login_test_user(&server, faker::internet::en::Username().fake::<String>()).await;

    let user2_id = get_string_thing(user_ident2.clone()).expect("user1");
    let ctx = Ctx::new(Ok(user_ident2.clone()), Uuid::new_v4(), false);

    let follow_db_service = FollowDbService {
        ctx: &ctx,
        db: &ctx_state._db,
    };

    let create_response = server
        .post(format!("/api/follow/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .json("")
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_success();

    let followers_nr = follow_db_service
        .user_followers_number(user1_id.clone())
        .await
        .expect("user 1 followers nr");
    assert_eq!(1, followers_nr);

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let post_stream_db_service = PostStreamDbService {
        ctx: &ctx,
        db: &ctx_state._db,
    };

    let streams = post_stream_db_service
        .user_posts_stream(user2_id.clone())
        .await;

    assert!(streams.is_ok());
    let post_streams = streams.unwrap();
    assert_eq!(post_streams.len(), 3);
    assert!(post_streams.contains(&get_string_thing(post_2).unwrap()));
    assert!(post_streams.contains(&get_string_thing(post_3).unwrap()));
    assert!(post_streams.contains(&get_string_thing(post_4).unwrap()));

    task.abort();
}
