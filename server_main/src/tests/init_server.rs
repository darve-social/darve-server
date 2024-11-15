
#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use sb_middleware::ctx::Ctx;
    use sb_user_auth::entity::local_user_entity::LocalUserDbService;
    use sb_user_auth::routes::init_server_routes::InitServerData;
    use sb_middleware::utils::db_utils::{UsernameIdent};
    use sb_user_auth::entity::access_right_entity::AccessRightDbService;
    use crate::test_utils::create_test_server;

    #[tokio::test]
    async fn init_server() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        // let (server,user_ident) = create_login_test_user(&server).await;
        let username = "username".to_string();
        let create_response = server.post("/init").json(&InitServerData { init_pass: "12dfas3".to_string(), username: username.clone(), password: "passs43flksfalsffas3".to_string(), email: "emm@us.com".to_string() }).await;
        // dbg!(&create_response);
        &create_response.assert_status_success();

        let ctx = &Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);
        let user = LocalUserDbService { db: &ctx_state._db, ctx }
            .get(UsernameIdent(username.clone()).into())
            .await;
        let user = user.unwrap();
        // dbg!(&user);
        let access_rights = AccessRightDbService{db: &ctx_state._db, ctx }.list_by_user(&user.id.unwrap()).await;
        assert_eq!(access_rights.unwrap().len()>0, true);
    }
}
