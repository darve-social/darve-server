#[cfg(test)]

mod tests{
    use sb_middleware::utils::string_utils::get_string_thing;

    use crate::test_utils::{create_login_test_user, create_test_server};

    #[tokio::test]
    async fn test_wallet_history(){
        // create test server
        println!("Creating test server");
        let (server, ctx_state) = create_test_server().await;

        let server = server.unwrap();
        
        // create 2 users with user1 and user2 names
        let username1 = "userrr1".to_string();
        let username2 = "userrr2".to_string();

        let (server, user_ident1) = create_login_test_user(&server, username1.clone()).await;
        
        let (server, user_ident2) = create_login_test_user(&server, username2.clone()).await;

        let user1_id = get_string_thing(user_ident1.clone()).expect("user1");
        let user2_id = get_string_thing(user_ident2.clone()).expect("user2");

        // endow using user2 by calling /api/dev/endow/:user_id/:amount
        let endow_user_response = server
            .get(&format!("/api/dev/endow/{}/{}",user1_id.to_string(),32))
            .add_header("Accept", "application/json")
            .json("")
            .await;

        let endow_response_text = endow_user_response.text();
        println!("endow_user_response: {}", endow_response_text);

        // check transaction history /api/user/wallet/history
        let transaction_history_response = server
            .get("/api/user/wallet/history")
            .add_header("Accept", "application/json")
            .await;

       transaction_history_response.assert_status_success();
        let response_text = transaction_history_response.text();
        println!("transaction_history_response: {}", response_text);

    }
}