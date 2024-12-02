
#[cfg(test)]
mod tests {
    use axum_test::multipart::MultipartForm;
    use uuid::Uuid;
    use sb_community::routes::profile_routes::{ProfileChat, ProfileChatList};
    use sb_middleware::ctx::Ctx;
    use sb_middleware::utils::request_utils::CreatedResponse;
    use sb_middleware::utils::string_utils::get_string_thing;
    use sb_user_auth::entity::follow_entitiy::FollowDbService;
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
        let (server, user_ident3) = create_login_test_user(&server, username3.clone()).await;

        let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);
        let follow_db_service = FollowDbService { ctx: &ctx, db: &ctx_state._db };
        let followers_nr = follow_db_service.user_followers_number(get_string_thing(user_ident2.clone()).unwrap()).await.expect("user 2 followers nr");
        assert_eq!(0, followers_nr);
        // logged in as username3
        let create_response = server.get(format!("/api/follow/{}",user_ident2.clone()).as_str()).await;
        let created = &create_response.json::<CreatedResponse>();
        assert_eq!(created.success, true);

        let followers_nr = follow_db_service.user_followers_number(get_string_thing(user_ident2).unwrap()).await.expect("user 2 followers nr");
        assert_eq!(1, followers_nr);



    }
}

