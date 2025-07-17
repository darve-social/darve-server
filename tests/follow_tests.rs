mod helpers;
use crate::helpers::{create_fake_login_test_user, create_login_test_user};
use axum_test::multipart::MultipartForm;
use darve_server::{
    entities::{
        community::{post_entity::Post, post_stream_entity::PostStreamDbService},
        user_auth::follow_entity,
        user_notification::UserNotification,
    },
    middleware,
    routes::{
        community::{
            discussion_routes::DiscussionPostView,
            profile_routes::{self, get_profile_community},
        },
        follows::UserItemView,
        user_auth::login_routes,
    },
    services::post_service::PostView,
};
use follow_entity::FollowDbService;
use helpers::{fake_username_min_len, post_helpers::create_fake_post};
use login_routes::LoginInput;
use middleware::ctx::Ctx;
use middleware::utils::string_utils::get_string_thing;
use profile_routes::ProfilePage;

test_with_server!(get_user_followers, |server, ctx_state, config| {
    let (server, user1, user1_pwd, _) = create_fake_login_test_user(&server).await;
    let (server, user2, _, _) = create_fake_login_test_user(&server).await;
    let user_ident1 = user1.id.as_ref().unwrap().to_raw();
    let username1 = user1.username.to_string();
    let username2 = user2.username.to_string();
    let user1_id = user1.id.unwrap();
    let user2_id = user2.id.unwrap();

    let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), false);
    let follow_db_service = FollowDbService {
        ctx: &ctx,
        db: &ctx_state.db.client,
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
        .post(format!("/api/followers/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .json("")
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_success();

    // refollow error
    let create_response = server
        .post(format!("/api/followers/{}", user_ident1.clone()).as_str())
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
    let (server, user3, user3_pwd, _) = create_fake_login_test_user(server).await;
    let username3 = user3.username;
    // follow u1
    let create_response = server
        .post(format!("/api/followers/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .json("")
        .await;
    create_response.assert_status_success();

    // refollow error
    let create_response = server
        .post(format!("/api/followers/{}", user_ident1.clone()).as_str())
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
        .get(format!("/api/followers/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_success();

    // check followers for user1
    let create_response = server
        .get(format!("/api/users/{}/followers", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<Vec<UserItemView>>();
    assert_eq!(created.len(), 2);
    let f_usernames: Vec<String> = created.iter().map(|fu| fu.username.clone()).collect();
    assert_eq!(f_usernames.contains(&username2.clone()), true);
    assert_eq!(f_usernames.contains(&username3.clone()), true);
    assert_eq!(f_usernames.contains(&username1.clone()), false);

    // user1 follows 0
    let create_response = server
        .get(format!("/api/users/{}/following", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<Vec<UserItemView>>();
    assert_eq!(created.len(), 0);

    // user3 get followers stream
    let create_response = server
        .get("/api/users/current/following/posts")
        .add_header("Accept", "application/json")
        .await;
    let posts = &create_response.json::<Vec<DiscussionPostView>>();
    assert_eq!(posts.len(), 0);

    // login user1
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

    // user1 post
    let post_name = "post title Name 1".to_string();
    let create_post = server
        .post("/api/users/current/posts")
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "contentttt")
                .add_text("topic_id", ""),
        )
        .add_header("Accept", "application/json")
        .await;
    create_post.assert_status_success();
    let _post = create_post.json::<Post>();

    let response = server
        .get("/api/users/current/posts")
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let posts = response.json::<Vec<PostView>>();

    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].created_by_name, username1);

    // login user3
    server.get("/logout").await;
    let login_response = server
        .post("/api/login")
        .json(&LoginInput {
            username: username3.clone(),
            password: user3_pwd.clone(),
            next: None,
        })
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_success();

    // user3 get followers stream
    let create_response = server
        .get("/api/users/current/following/posts")
        .add_header("Accept", "application/json")
        .await;
    let posts = &create_response.json::<Vec<DiscussionPostView>>();
    assert_eq!(posts.len(), 1);

    // login user1
    server
        .get("/logout")
        .add_header("Accept", "application/json")
        .await;
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

    // user1 post 2
    let post_name = "post title Name 2".to_string();
    let create_post = server
        .post("/api/users/current/posts")
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "contentttt22")
                .add_text("topic_id", ""),
        )
        .add_header("Accept", "application/json")
        .await;
    create_post.assert_status_success();

    // login user3
    server.get("/logout").await;
    let login_response = server
        .post("/api/login")
        .json(&LoginInput {
            username: username3.clone(),
            password: user3_pwd.clone(),
            next: None,
        })
        .await;
    login_response.assert_status_success();

    // user3 get followers stream
    let create_response = server
        .get("/api/users/current/following/posts")
        .add_header("Accept", "application/json")
        .await;
    let posts = &create_response.json::<Vec<DiscussionPostView>>();
    assert_eq!(posts.len(), 2);

    // user3 unfollow user1
    let create_response = server
        .delete(format!("/api/followers/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_success();

    // check nr of user1 followers
    let profile1_response = server
        .get(format!("/u/{}", username1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    let created = profile1_response.json::<ProfilePage>();
    assert_eq!(created.profile_view.unwrap().followers_nr, 1);

    // check nr of user1 followers
    let create_response = server
        .get(format!("/api/users/{}/followers", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    let created = &create_response.json::<Vec<UserItemView>>();
    assert_eq!(created.len(), 1);

    // check user3 unfollowed user1
    let create_response = server
        .get(format!("/api/followers/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .await;
    create_response.assert_status_success();

    // login user1
    server.get("/logout").await;
    let login_response = server
        .post("/api/login")
        .json(&LoginInput {
            username: username1.clone(),
            password: user1_pwd.clone(),
            next: None,
        })
        .await;
    login_response.assert_status_success();

    // user1 post 3
    let post_name = "post title Name 3".to_string();
    let create_post = server
        .post("/api/users/current/posts")
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "contentttt3")
                .add_text("topic_id", ""),
        )
        .add_header("Accept", "application/json")
        .await;
    create_post.assert_status_success();

    // login user3
    server.get("/logout").await;
    let login_response = server
        .post("/api/login")
        .json(&LoginInput {
            username: username3.clone(),
            password: user3_pwd.clone(),
            next: None,
        })
        .add_header("Accept", "application/json")
        .await;
    login_response.assert_status_success();

    // user3 get followers stream
    let create_response = server
        .get("/api/users/current/following/posts")
        .add_header("Accept", "application/json")
        .await;
    let posts = &create_response.json::<Vec<DiscussionPostView>>();
    assert_eq!(posts.len(), 2);

    let notifications_response = server
        .get("/api/notifications")
        .add_header("Accept", "application/json")
        .await;
    notifications_response.assert_status_success();
    let notifications = notifications_response.json::<Vec<UserNotification>>();
    assert_eq!(notifications.len(), 2)
});

test_with_server!(
    add_latest_three_posts_of_follower_to_ctx_user,
    |server, ctx_state, config| {
        let (_, user_ident1) = create_login_test_user(&server, fake_username_min_len(6)).await;

        let user1_id = get_string_thing(user_ident1.clone()).expect("user1");
        let ctx = Ctx::new(Ok(user_ident1.clone()), false);

        let profile_discussion =
            get_profile_community(&ctx_state.db.client, &ctx, user1_id.clone())
                .await
                .unwrap()
                .default_discussion
                .unwrap();

        let _ = create_fake_post(&server, &profile_discussion, None, None).await;
        let post_2 = create_fake_post(&server, &profile_discussion, None, None).await;
        let post_3 = create_fake_post(&server, &profile_discussion, None, None).await;
        let post_4 = create_fake_post(&server, &profile_discussion, None, None).await;

        let (_, user_ident2) = create_login_test_user(&server, fake_username_min_len(6)).await;

        let user2_id = get_string_thing(user_ident2.clone()).expect("user1");
        let ctx = Ctx::new(Ok(user_ident2.clone()), false);

        let follow_db_service = FollowDbService {
            ctx: &ctx,
            db: &ctx_state.db.client,
        };

        let create_response = server
            .post(format!("/api/followers/{}", user_ident1.clone()).as_str())
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
            db: &ctx_state.db.client,
        };

        let streams = post_stream_db_service
            .get_posts::<PostView>(user2_id.clone())
            .await;
        println!(">>>>>>>{:?}", streams);
        let post_streams = streams
            .unwrap()
            .iter()
            .map(|p| p.id.to_raw())
            .collect::<Vec<String>>();

        assert_eq!(post_streams.len(), 3);
        assert!(post_streams.contains(&post_2.id));
        assert!(post_streams.contains(&post_3.id));
        assert!(post_streams.contains(&post_4.id));
    }
);
