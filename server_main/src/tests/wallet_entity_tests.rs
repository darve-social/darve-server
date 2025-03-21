#[cfg(test)]

mod tests{
    use crate::test_utils::{create_login_test_user, create_test_server};

    #[tokio::test]
    async fn test_wallet_history(){
        // create test server
        println!("Creating test server");
        let (server, ctx_state) = create_test_server().await;

        let server = server.unwrap();

        let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;

        // generate payment intent : /api/user/wallet/endowment/:amount
        let generate_payment_intent = server
            .get("/api/user/wallet/endowment/134")
            .add_header("Accept", "application/json")
            .await;
        
        let response_body = generate_payment_intent.text();
        println!("Response Body: {}", response_body);
        // use secret key to make transaction
        let transaction = server
            .post(&format!("/api/stripe/test/payment/134/{}", response_body))
            .add_header("Accept", "application/json")
            .await;

        let transaction_response_body = transaction.text();
        println!("Response Body: {}", transaction_response_body);

        // fetch tx history : /api/user/wallet/history
        let tx_history_response = server
            .get("/api/user/wallet/history")
            .add_header("Accept", "application/json")
            .await;
    }
}