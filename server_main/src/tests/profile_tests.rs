#[cfg(test)]
mod tests {
    use crate::test_utils::{create_login_test_user, create_test_server};
    use axum_test::multipart::MultipartForm;
    use sb_community::routes::discussion_routes::get_discussion_view;
    use sb_community::routes::profile_routes::{ProfileChat, ProfileChatList, SearchInput};
    use sb_middleware::ctx::Ctx;
    use sb_middleware::error::AppError;
    use sb_middleware::utils::extractor_utils::DiscussionParams;
    use sb_middleware::utils::request_utils::CreatedResponse;
    use sb_user_auth::routes::follow_routes::UserListView;
    use sb_user_auth::routes::login_routes::LoginInput;
    use uuid::Uuid;

    #[tokio::test]
    async fn search_users() {
        let (server, ctx_state) = create_test_server().await;
        let username1 = "its_user_one".to_string();
        let username2 = "its_user_two".to_string();
        let username3 = "herodus".to_string();
        let (server, user_ident1) = create_login_test_user(&server, username1.clone()).await;
        let (server, user_ident2) = create_login_test_user(&server, username2.clone()).await;
        let (server, user_ident3) = create_login_test_user(&server, username3.clone()).await;

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

        let request = server
            .post("/api/user/search")
            .json(&SearchInput {
                query: "rset".to_string(),
            })
            .add_header("Accept", "application/json")
            .await;
        request.assert_status_success();
        let res = &request.json::<UserListView>();
        assert_eq!(res.items.len(), 0);
        let request = server
            .post("/api/user/search")
            .json(&SearchInput {
                query: "Userset".to_string(),
            })
            .add_header("Accept", "application/json")
            .await;
        request.assert_status_success();
        let res = &request.json::<UserListView>();
        assert_eq!(res.items.len(), 1);

        let request = server
            .post("/api/user/search")
            .json(&SearchInput {
                query: "one".to_string(),
            })
            .add_header("Accept", "application/json")
            .await;
        request.assert_status_success();
        let res = &request.json::<UserListView>();
        assert_eq!(res.items.len(), 1);

        let request = server
            .post("/api/user/search")
            .json(&SearchInput {
                query: "unknown".to_string(),
            })
            .add_header("Accept", "application/json")
            .await;
        request.assert_status_success();
        let res = &request.json::<UserListView>();
        assert_eq!(res.items.len(), 0);

        let request = server
            .post("/api/user/search")
            .json(&SearchInput {
                query: "its".to_string(),
            })
            .add_header("Accept", "application/json")
            .await;

        request.assert_status_success();
        let res = &request.json::<UserListView>();
        assert_eq!(res.items.len(), 2);
    }

    #[tokio::test]
    async fn get_user_chat() {
        let (server, ctx_state) = create_test_server().await;
        let username1 = "usnnnn".to_string();
        let username2 = "usnnnn2".to_string();
        let username3 = "usnnnn3".to_string();
        let (server, user_ident1) = create_login_test_user(&server, username1.clone()).await;
        let (server, user_ident2) = create_login_test_user(&server, username2.clone()).await;

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
        &create_response.assert_status_success();
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
        &create_post.assert_status_success();
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

        // login username1
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
        &create_response.assert_status_success();
        let created = &create_response.json::<ProfileChat>();
        let chat_disc_id_usr1 = created.discussion.id.clone().unwrap();
        assert_eq!(chat_disc_id_usr1, chat_disc_id.clone());

        let ctx = Ctx::new(Ok(user_ident1), Uuid::new_v4(), false);
        let discussion_posts = get_discussion_view(
            &ctx_state._db,
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
        let (server, user_ident3) = create_login_test_user(&server, username3.clone()).await;
        let ctx = Ctx::new(Ok(user_ident3), Uuid::new_v4(), false);
        let d_view = get_discussion_view(
            &ctx_state._db,
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

        let post_name = "post title Name 2".to_string();
        let create_post = server
            .post(format!("/api/discussion/{chat_disc_id}/post").as_str())
            .multipart(
                MultipartForm::new()
                    .add_text("title", post_name.clone())
                    .add_text("content", "contentttt2")
                    .add_text("topic_id", ""),
            )
            .add_header("Accept", "application/json")
            .await;
        dbg!(&create_post);
        &create_post.assert_status_failure();
    }
}
