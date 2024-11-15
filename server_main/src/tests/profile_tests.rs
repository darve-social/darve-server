
#[cfg(test)]
mod tests {
    use axum_test::multipart::MultipartForm;
    use sb_community::routes::profile_routes::{ProfileChat, ProfileChatList};
    use sb_middleware::utils::request_utils::CreatedResponse;
    use sb_user_auth::routes::login_routes::LoginInput;
    use crate::test_utils::{create_login_test_user, create_test_server};

    #[tokio::test]
    async fn get_user_chat() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let username1 = "usnnnn".to_string();
        let username2 = "usnnnn2".to_string();
        let (server, user_ident1) = create_login_test_user(&server, username1.clone()).await;
        let (server, user_ident2) = create_login_test_user(&server, username2.clone()).await;

        // logged in as username2
        let create_response = server.get("/api/user_chat/list").await;
        let created = &create_response.json::<ProfileChatList>();
        assert_eq!(created.discussions.len(), 0);
        assert_eq!(created.user_id.to_raw(), user_ident2);

        let create_response = server.get(format!("/api/user_chat/with/{}", user_ident1.as_str()).as_str()).await;
        &create_response.assert_status_success();
        let created = &create_response.json::<ProfileChat>();
        let chat_disc_id = created.discussion.id.clone().unwrap();

        let post_name = "post title Name 1".to_string();
        let create_post = server.post(format!("/api/discussion/{chat_disc_id}/post").as_str()).multipart(MultipartForm::new().add_text("title", post_name.clone()).add_text("content", "contentttt").add_text("topic_id", "")).await;
        let created = create_post.json::<CreatedResponse>();
        &create_post.assert_status_success();
        assert_eq!(created.id.len() > 0, true);

        let create_response = server.get("/api/user_chat/list").await;
        let created = &create_response.json::<ProfileChatList>();
        assert_eq!(created.discussions.len(), 1);
        assert_eq!(created.user_id.to_raw(), user_ident2);
        let list_disc_id = created.discussions.get(0).unwrap().id.clone().unwrap();
        assert_eq!(list_disc_id, chat_disc_id);

        server.get("/logout").await;
        let login_response = server.post("/api/login").json(&LoginInput { username: username1.clone(), password: "some3242paSs#$".to_string(), next: None }).await;
        login_response.assert_status_success();
        // logged in as username1


        let create_response = server.get("/api/user_chat/list").await;
        create_response.assert_status_success();
        let created = &create_response.json::<ProfileChatList>();
        assert_eq!(created.discussions.len(), 1);
        assert_eq!(created.user_id.to_raw(), user_ident1);
        let list_disc_id = created.discussions.get(0).unwrap().id.clone().unwrap();
        assert_eq!(list_disc_id, chat_disc_id);

        let create_response = server.get(format!("/api/user_chat/with/{}", user_ident2.as_str()).as_str()).await;
        &create_response.assert_status_success();
        let created = &create_response.json::<ProfileChat>();
        let chat_disc_id_usr1 = created.discussion.id.clone().unwrap();
        assert_eq!(chat_disc_id_usr1, chat_disc_id);
    }
}

