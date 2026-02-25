mod helpers;
use crate::helpers::create_fake_login_test_user;
use axum_test::multipart::MultipartForm;
use darve_server::{
    entities::{community::discussion_entity::DiscussionDbService, user_auth::follow_entity},
    middleware,
    models::view::post::PostView,
    routes::{community::profile_routes::ProfileView, follows::UserItemView},
};
use follow_entity::FollowDbService;
use middleware::ctx::Ctx;

test_with_server!(get_user_followers, |server, ctx_state, config| {
    let (server, user1, user1_pwd, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;
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
        .add_header("Authorization", format!("Bearer {}", token2))
        .await;
    let created = profile1_response.json::<ProfileView>();
    assert_eq!(created.followers_nr, 0);

    // logged in as username2
    // follow user_ident1
    let create_response = server
        .post(format!("/api/following/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .json("")
        .add_header("Authorization", format!("Bearer {}", token2))
        .await;
    create_response.assert_status_success();

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
    let created = profile1_response.json::<ProfileView>();
    assert_eq!(created.followers_nr, 1);

    //login as username3
    let (server, user3, _, token3) = create_fake_login_test_user(server).await;
    let username3 = user3.username;
    // follow u1
    let create_response = server
        .post(format!("/api/following/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .json("")
        .add_header("Authorization", format!("Bearer {}", token3))
        .await;
    create_response.assert_status_success();

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
        .add_header("Authorization", format!("Bearer {}", token3))
        .await;
    let created = profile1_response.json::<ProfileView>();
    assert_eq!(created.followers_nr, 2);

    // check if follows user1
    let create_response = server
        .get(format!("/api/following/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;
    create_response.assert_status_success();

    // check followers for user1
    let create_response = server
        .get(format!("/api/users/{}/followers", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token2))
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
        .add_header("Authorization", format!("Bearer {}", token3))
        .await;
    let created = &create_response.json::<Vec<UserItemView>>();
    assert_eq!(created.len(), 0);

    // user3 get followers stream
    let create_response = server
        .get("/api/users/current/following/posts")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token3))
        .await;
    let posts = &create_response.json::<Vec<PostView>>();
    assert_eq!(posts.len(), 0);

    // login user1
    server.get("/logout").await;
    let login_response = server
        .post("/api/login")
        .add_header("Accept", "application/json")
        .json(&serde_json::json!({
            "username_or_email": username1.clone(),
            "password": user1_pwd.clone()
        }))
        .await;
    login_response.assert_status_success();

    // user1 post
    let disc_id = DiscussionDbService::get_profile_discussion_id(&user1_id).to_raw();
    let post_name = "post title Name 1".to_string();
    let create_post = server
        .post(&format!("/api/discussions/{disc_id}/posts"))
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "contentttt"),
        )
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;
    create_post.assert_status_success();
    let _post = create_post.json::<PostView>();

    let response = server
        .get(&format!("/api/discussions/{disc_id}/posts"))
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;
    response.assert_status_success();
    let posts = response.json::<Vec<PostView>>();

    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].created_by.username, username1);

    // user3 get followers stream
    let create_response = server
        .get("/api/users/current/following/posts")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token3))
        .await;

    let posts = &create_response.json::<Vec<PostView>>();
    assert_eq!(posts.len(), 1);

    // user1 post 2
    let post_name = "post title Name 2".to_string();
    let disc_id = DiscussionDbService::get_profile_discussion_id(&user1_id).to_raw();
    let create_post = server
        .post(&format!("/api/discussions/{disc_id}/posts"))
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "contentttt22"),
        )
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;
    create_post.assert_status_success();

    // user3 get followers stream
    let create_response = server
        .get("/api/users/current/following/posts")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token3))
        .await;
    let posts = &create_response.json::<Vec<PostView>>();
    assert_eq!(posts.len(), 2);

    // user3 unfollow user1
    let create_response = server
        .delete(format!("/api/following/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token3))
        .await;
    create_response.assert_status_success();

    // check nr of user1 followers
    let profile1_response = server
        .get(format!("/u/{}", username1.clone()).as_str())
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;
    let created = profile1_response.json::<ProfileView>();
    assert_eq!(created.followers_nr, 1);

    // check nr of user1 followers
    let create_response = server
        .get(format!("/api/users/{}/followers", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;
    let created = &create_response.json::<Vec<UserItemView>>();
    assert_eq!(created.len(), 1);

    // check user3 unfollowed user1
    let create_response = server
        .get(format!("/api/following/{}", user_ident1.clone()).as_str())
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;
    create_response.assert_status_success();

    // user1 post 3
    let post_name = "post title Name 3".to_string();
    let disc_id = DiscussionDbService::get_profile_discussion_id(&user1_id).to_raw();
    let create_post = server
        .post(&format!("/api/discussions/{disc_id}/posts"))
        .multipart(
            MultipartForm::new()
                .add_text("title", post_name.clone())
                .add_text("content", "contentttt3"),
        )
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;
    create_post.assert_status_success();

    // user3 get followers stream
    let create_response = server
        .get("/api/users/current/following/posts")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token3))
        .await;
    let posts = &create_response.json::<Vec<PostView>>();
    assert_eq!(posts.len(), 0);
});

test_with_server!(remove_followers, |server, ctx_state, config| {
    let (server, user1, __, token1) = create_fake_login_test_user(&server).await;
    let (server, user2, _, token) = create_fake_login_test_user(&server).await;
    let create_response = server
        .post(format!("/api/following/{}", user1.id.as_ref().unwrap().to_raw()).as_str())
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token))
        .await;
    create_response.assert_status_success();

    let is_following_response = server
        .get(&format!(
            "/api/following/{}",
            user1.id.as_ref().unwrap().to_raw()
        ))
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token))
        .await;
    is_following_response.assert_status_success();
    assert!(is_following_response.json::<bool>());

    let remove_response = server
        .delete(&format!(
            "/api/followers/{}",
            user2.id.as_ref().unwrap().to_raw()
        ))
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .await;
    remove_response.assert_status_success();
    let is_following_response = server
        .get(&format!(
            "/api/following/{}",
            user1.id.as_ref().unwrap().to_raw()
        ))
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;
    is_following_response.assert_status_success();
    assert_eq!(is_following_response.json::<bool>(), false);
});
