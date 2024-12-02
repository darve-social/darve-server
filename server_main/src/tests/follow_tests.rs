
#[cfg(test)]
mod tests {
    use axum_test::multipart::MultipartForm;
    use uuid::Uuid;
    use sb_community::routes::profile_routes::{ProfileChat, ProfileChatList, ProfilePage};
    use sb_middleware::ctx::Ctx;
    use sb_middleware::utils::request_utils::CreatedResponse;
    use sb_middleware::utils::string_utils::get_string_thing;
    use sb_user_auth::entity::follow_entitiy::FollowDbService;
    use sb_user_auth::routes::follow_routes::FollowUserList;
    use sb_user_auth::routes::login_routes::LoginInput;
    use crate::test_utils::{create_login_test_user, create_test_server};

    #[tokio::test]
    async fn get_user_followers() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let username1 = "usnnnn".to_string();
        let username2 = "usnnnn2".to_string();
        let username3 = "usnnnn3".to_string();
        let (server, user_ident1) = create_login_test_user(&server, username1.clone()).await;
        let (server, user_ident2) = create_login_test_user(&server, username2.clone()).await;

        let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);
        let follow_db_service = FollowDbService { ctx: &ctx, db: &ctx_state._db };
        let followers_nr = follow_db_service.user_followers_number(get_string_thing(user_ident1.clone()).unwrap()).await.expect("user 1 followers nr");
        assert_eq!(0, followers_nr);

        let is_following = follow_db_service.is_following(get_string_thing(user_ident1.clone()).expect("user"), get_string_thing(user_ident2.clone()).expect("user")).await.expect("is_following");
        assert_eq!(is_following, false);

        let profile1_response =server.get(format!("/u/{}",username1.clone()).as_str()).await;
        let created = profile1_response.json::<ProfilePage>();
        assert_eq!(created.profile_view.unwrap().followers_nr, 0);

        // logged in as username2
        // follow user_ident1
        let create_response = server.post(format!("/api/follow/{}",user_ident1.clone()).as_str()).json("").await;
        let created = &create_response.json::<CreatedResponse>();
        assert_eq!(created.success, true);

        // refollow error
        let create_response = server.post(format!("/api/follow/{}",user_ident1.clone()).as_str()).json("").await;
        create_response.assert_status_failure();

        let followers_nr = follow_db_service.user_followers_number(get_string_thing(user_ident1.clone()).unwrap()).await.expect("user 1 followers nr");
        assert_eq!(1, followers_nr);

        let is_following = follow_db_service.is_following(get_string_thing(user_ident1.clone()).expect("user"), get_string_thing(user_ident2.clone()).expect("user")).await.expect("is_following");
        assert_eq!(is_following, false);
        let is_following = follow_db_service.is_following(get_string_thing(user_ident2.clone()).expect("user"), get_string_thing(user_ident1.clone()).expect("user")).await.expect("is_following");
        assert_eq!(is_following, true);

        let profile1_response =server.get(format!("/u/{}",username1.clone()).as_str()).await;
        let created = profile1_response.json::<ProfilePage>();
        assert_eq!(created.profile_view.unwrap().followers_nr, 1);

        //login as username3
        let (server, user_ident3) = create_login_test_user(&server, username3.clone()).await;

        // follow u1
        let create_response = server.post(format!("/api/follow/{}",user_ident1.clone()).as_str()).json("").await;
        let created = &create_response.json::<CreatedResponse>();
        assert_eq!(created.success, true);

        // refollow error
        let create_response = server.post(format!("/api/follow/{}",user_ident1.clone()).as_str()).json("").await;
        create_response.assert_status_failure();

        // check nr of followers
        let followers_nr = follow_db_service.user_followers_number(get_string_thing(user_ident1.clone()).unwrap()).await.expect("user 1 followers nr");
        assert_eq!(2, followers_nr);

        let profile1_response =server.get(format!("/u/{}",username1.clone()).as_str()).await;
        let created = profile1_response.json::<ProfilePage>();
        assert_eq!(created.profile_view.unwrap().followers_nr, 2);

        let create_response = server.get(format!("/api/user/follows/{}",user_ident1.clone()).as_str()).await;
        let created = &create_response.json::<CreatedResponse>();
        assert_eq!(created.success, true);

        let create_response = server.get(format!("/api/user/{}/followers",user_ident1.clone()).as_str()).await;
        let created = &create_response.json::<FollowUserList>();
        assert_eq!(created.list.len(), 2);
        let f_usernames: Vec<String> = created.list.iter().map(|fu| fu.username.clone()).collect();
        assert_eq!(f_usernames.contains( &username2.clone()),true);
        assert_eq!(f_usernames.contains( &username3.clone()),true);
        assert_eq!(f_usernames.contains( &username1.clone()),false);

        let create_response = server.get(format!("/api/user/{}/following",user_ident1.clone()).as_str()).await;
        let created = &create_response.json::<FollowUserList>();
        assert_eq!(created.list.len(), 0);

        // unfollow
        let create_response = server.delete(format!("/api/follow/{}",user_ident1.clone()).as_str()).await;
        let created = &create_response.json::<CreatedResponse>();
        assert_eq!(created.success, true);

        let profile1_response =server.get(format!("/u/{}",username1.clone()).as_str()).await;
        let created = profile1_response.json::<ProfilePage>();
        assert_eq!(created.profile_view.unwrap().followers_nr, 1);

        let create_response = server.get(format!("/api/user/{}/followers",user_ident1.clone()).as_str()).await;
        let created = &create_response.json::<FollowUserList>();
        assert_eq!(created.list.len(), 1);

        let create_response = server.get(format!("/api/user/follows/{}",user_ident1.clone()).as_str()).await;
        let created = &create_response.json::<CreatedResponse>();
        assert_eq!(created.success, false);

    }
}

