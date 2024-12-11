#[cfg(test)]
mod tests {
    use crate::test_utils::{create_login_test_user, create_test_server};
    use axum_test::multipart::MultipartForm;
    use tokio::io::AsyncWriteExt;
    use tokio_stream::StreamExt;
    use sb_community::routes::discussion_routes::DiscussionView;
    use sb_community::routes::profile_routes::{FollowingStreamView, ProfileDiscussionView, ProfilePage};
    use sb_middleware::ctx::Ctx;
    use sb_middleware::utils::request_utils::CreatedResponse;
    use sb_middleware::utils::string_utils::get_string_thing;
    use sb_user_auth::entity::follow_entitiy::FollowDbService;
    use sb_user_auth::routes::follow_routes::FollowUserList;
    use sb_user_auth::routes::login_routes::LoginInput;
    use uuid::Uuid;
    use sb_middleware::db;

    #[tokio::test]
    async fn get_user_followers() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
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
            .await;
        let created = profile1_response.json::<ProfilePage>();
        assert_eq!(created.profile_view.unwrap().followers_nr, 0);

        // logged in as username2
        // follow user_ident1
        let create_response = server
            .post(format!("/api/follow/{}", user_ident1.clone()).as_str())
            .json("")
            .await;
        let created = &create_response.json::<CreatedResponse>();
        assert_eq!(created.success, true);

        // refollow error
        let create_response = server
            .post(format!("/api/follow/{}", user_ident1.clone()).as_str())
            .json("")
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
            .await;
        let created = profile1_response.json::<ProfilePage>();
        assert_eq!(created.profile_view.unwrap().followers_nr, 1);

        //login as username3
        let (server, user_ident3) = create_login_test_user(&server, username3.clone()).await;

        // follow u1
        let create_response = server
            .post(format!("/api/follow/{}", user_ident1.clone()).as_str())
            .json("")
            .await;
        let created = &create_response.json::<CreatedResponse>();
        assert_eq!(created.success, true);

        // refollow error
        let create_response = server
            .post(format!("/api/follow/{}", user_ident1.clone()).as_str())
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
            .await;
        let created = profile1_response.json::<ProfilePage>();
        assert_eq!(created.profile_view.unwrap().followers_nr, 2);

        // check if follows user1
        let create_response = server
            .get(format!("/api/user/follows/{}", user_ident1.clone()).as_str())
            .await;
        let created = &create_response.json::<CreatedResponse>();
        assert_eq!(created.success, true);

        // check followers for user1
        let create_response = server
            .get(format!("/api/user/{}/followers", user_ident1.clone()).as_str())
            .await;
        let created = &create_response.json::<FollowUserList>();
        assert_eq!(created.list.len(), 2);
        let f_usernames: Vec<String> = created.list.iter().map(|fu| fu.username.clone()).collect();
        assert_eq!(f_usernames.contains(&username2.clone()), true);
        assert_eq!(f_usernames.contains(&username3.clone()), true);
        assert_eq!(f_usernames.contains(&username1.clone()), false);

        // user1 follows 0
        let create_response = server
            .get(format!("/api/user/{}/following", user_ident1.clone()).as_str())
            .await;
        let created = &create_response.json::<FollowUserList>();
        assert_eq!(created.list.len(), 0);

        // user3 get followers stream
        let create_response = server.get("/u/following/posts").await;
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
            .await;
        let created = create_post.json::<CreatedResponse>();
        &create_post.assert_status_success();
        assert_eq!(created.id.len() > 0, true);

        let response = server
            .get(format!("/api/user/{}/posts", username1).as_str())
            .await;
        let parsed_response = response.json::<ProfileDiscussionView>();
        &create_post.assert_status_success();
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
            .await;
        login_response.assert_status_success();

        // user3 get followers stream
        let create_response = server.get("/u/following/posts").await;
        let created = &create_response.json::<FollowingStreamView>();
        assert_eq!(created.post_list.len(), 1);

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
            .await;
        let created = create_post.json::<CreatedResponse>();
        &create_post.assert_status_success();
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
        let create_response = server.get("/u/following/posts").await;
        let created = &create_response.json::<FollowingStreamView>();
        assert_eq!(created.post_list.len(), 2);

        // user3 unfollow user1
        let create_response = server
            .delete(format!("/api/follow/{}", user_ident1.clone()).as_str())
            .await;
        let created = &create_response.json::<CreatedResponse>();
        assert_eq!(created.success, true);

        // check nr of user1 followers
        let profile1_response = server
            .get(format!("/u/{}", username1.clone()).as_str())
            .await;
        let created = profile1_response.json::<ProfilePage>();
        assert_eq!(created.profile_view.unwrap().followers_nr, 1);

        // check nr of user1 followers
        let create_response = server
            .get(format!("/api/user/{}/followers", user_ident1.clone()).as_str())
            .await;
        let created = &create_response.json::<FollowUserList>();
        assert_eq!(created.list.len(), 1);

        // check user3 unfollowed user1
        let create_response = server
            .get(format!("/api/user/follows/{}", user_ident1.clone()).as_str())
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
            .await;
        let created = create_post.json::<CreatedResponse>();
        &create_post.assert_status_success();
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
        let create_response = server.get("/u/following/posts").await;
        let created = &create_response.json::<FollowingStreamView>();
        assert_eq!(created.post_list.len(), 2);
    }

}
