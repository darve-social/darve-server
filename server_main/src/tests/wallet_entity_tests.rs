#[cfg(test)]

mod tests {
    use crate::test_utils::{create_login_test_user, create_test_server};
    use sb_middleware::utils::string_utils::get_string_thing;
    use sb_wallet::routes::wallet_routes::CurrencyTransactionHistoryView;

    #[tokio::test]
    async fn test_wallet_history() {
        // create test server
        println!("Creating test server");
        let (server, _ctx_state) = create_test_server().await;

        // create 2 users with user1 and user2 names
        let username1 = "userrr1".to_string();
        let username2 = "userrr2".to_string();

        let (server, user_ident1) = create_login_test_user(&server, username1.clone()).await;

        let (server, user_ident2) = create_login_test_user(&server, username2.clone()).await;

        let user1_id = get_string_thing(user_ident1.clone()).expect("user1");
        let user2_id = get_string_thing(user_ident2.clone()).expect("user2");

        // endow using user2 by calling /api/dev/endow/:user_id/:amount
        let endow_amt = 32;
        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user2_id.to_string(),
                endow_amt
            ))
            .add_header("Accept", "application/json")
            .json("")
            .await;

        endow_user_response.assert_status_success();
        let endow_response_text = endow_user_response.text();
        println!("endow_user_response: {}", endow_response_text);

        // check transaction history /api/user/wallet/history
        let transaction_history_response = server
            .get("/api/user/wallet/history")
            .add_header("Accept", "application/json")
            .await;

        transaction_history_response.assert_status_success();
        dbg!(&transaction_history_response);

        let created = &transaction_history_response.json::<CurrencyTransactionHistoryView>();
        assert_eq!(created.transactions.len(), 1);
        assert_eq!(
            created.transactions.get(0).unwrap().amount_in,
            Some(endow_amt)
        );
    }
}
