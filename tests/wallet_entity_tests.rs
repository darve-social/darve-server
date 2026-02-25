mod helpers;
use darve_server::entities::wallet::wallet_entity::WalletBalancesView;
use darve_server::middleware;
use darve_server::{
    entities::wallet::gateway_transaction_entity::GatewayTransaction,
    models::view::balance_tx::CurrencyTransactionView,
};
use middleware::utils::string_utils::get_string_thing;
use serde_json::json;

use crate::helpers::create_fake_login_test_user;

test_with_server!(test_wallet_history, |server, ctx_state, config| {
    // create 2 users
    let (server, user1, _, _token) = create_fake_login_test_user(&server).await;
    let user_ident1 = user1.id.unwrap().to_raw();

    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;
    let username2 = user2.username.clone();
    let _ = get_string_thing(user_ident1.clone()).expect("user1");

    let endow_amt = 32;
    let endow_user_response = server
        .get(&format!("/test/api/deposit/{}/{}", username2, endow_amt))
        .add_header("Accept", "application/json")
        .json("")
        .await;

    let endow_response_text = endow_user_response.text();
    println!("endow_user_response: {}", endow_response_text);
    endow_user_response.assert_status_success();

    // check transaction history /api/user/wallet/history
    let transaction_history_response = server
        .get("/api/wallet/history")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token2))
        .await;

    transaction_history_response.assert_status_success();

    let transactions = &transaction_history_response.json::<Vec<CurrencyTransactionView>>();
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions.get(0).unwrap().amount_in, Some(endow_amt));
});

test_with_server!(check_balance_too_low, |server, ctx_state, config| {
    let (server, user2, _, token) = create_fake_login_test_user(&server).await;

    ctx_state
        .db
        .client
        .query("UPDATE $user SET email_verified=$email;")
        .bind(("user", user2.id.as_ref().unwrap().clone()))
        .bind(("email", "text@text.com"))
        .await
        .unwrap();

    let endow_amt = 32;
    let endow_user_response = server
        .get(&format!(
            "/test/api/deposit/{}/{}",
            user2.username, endow_amt
        ))
        .add_header("Accept", "application/json")
        .json("")
        .await;

    let endow_response_text = endow_user_response.text();
    println!("endow_user_response: {}", endow_response_text);
    endow_user_response.assert_status_success();

    let res = server
        .post("/api/wallet/withdraw")
        .add_header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "amount": 100 }))
        .await;

    res.assert_status_failure();

    assert!(res
        .text()
        .contains(&middleware::error::AppError::BalanceTooLow.to_string()));
});

test_with_server!(prod_balance_0, |server, ctx_state, config| {
    let (_, _user, _password, token) = create_fake_login_test_user(&server).await;
    let response = server
        .get("/api/wallet/balance")
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let balances = response.json::<WalletBalancesView>();
    assert_eq!(balances.balance_locked.balance_usd, 0);
});

test_with_server!(test_gateway_wallet_history, |server, ctx_state, config| {
    let (server, user, _, token) = create_fake_login_test_user(&server).await;

    let endow_amt = 32;
    let endow_user_response = server
        .get(&format!(
            "/test/api/deposit/{}/{}",
            user.username, endow_amt
        ))
        .add_header("Authorization", format!("Bearer {}", token))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    endow_user_response.assert_status_success();

    let endow_user_response = server
        .get(&format!(
            "/test/api/deposit/{}/{}",
            user.username, endow_amt
        ))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    endow_user_response.assert_status_success();

    let endow_user_response = server
        .get(&format!(
            "/test/api/deposit/{}/{}",
            user.username, endow_amt
        ))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    endow_user_response.assert_status_success();

    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;

    let endow_amt = 32;
    let endow_user_response = server
        .get(&format!(
            "/test/api/deposit/{}/{}",
            user1.username, endow_amt
        ))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    endow_user_response.assert_status_success();

    let endow_user_response = server
        .get(&format!(
            "/test/api/deposit/{}/{}",
            user1.username, endow_amt
        ))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    endow_user_response.assert_status_success();

    let transaction_history_response = server
        .get("/api/gateway_wallet/history")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token))
        .await;

    transaction_history_response.assert_status_success();

    let transactions = &transaction_history_response.json::<Vec<GatewayTransaction>>();
    assert_eq!(transactions.len(), 3);

    let transaction_history_response = server
        .get("/api/gateway_wallet/history?type=Deposit")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token))
        .await;

    transaction_history_response.assert_status_success();

    let transactions = &transaction_history_response.json::<Vec<GatewayTransaction>>();
    assert_eq!(transactions.len(), 3);

    let transaction_history_response = server
        .get("/api/gateway_wallet/history?type=Withdraw")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token))
        .await;

    transaction_history_response.assert_status_success();

    let transactions = &transaction_history_response.json::<Vec<GatewayTransaction>>();
    assert_eq!(transactions.len(), 0);

    let transaction_history_response = server
        .get("/api/gateway_wallet/history")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;

    transaction_history_response.assert_status_success();

    let transactions = &transaction_history_response.json::<Vec<GatewayTransaction>>();
    assert_eq!(transactions.len(), 2);

    let transaction_history_response = server
        .get("/api/gateway_wallet/history?status=Failed")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;

    transaction_history_response.assert_status_success();

    let transactions = &transaction_history_response.json::<Vec<GatewayTransaction>>();
    assert_eq!(transactions.len(), 0);

    let transaction_history_response = server
        .get("/api/gateway_wallet/history?status=Completed")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;

    transaction_history_response.assert_status_success();

    let transactions = &transaction_history_response.json::<Vec<GatewayTransaction>>();
    assert_eq!(transactions.len(), 2);
    let transaction_count_response = server
        .get("/api/gateway_wallet/count?status=Completed")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;
    transaction_count_response.assert_status_success();
    let count = transaction_count_response.json::<u64>();
    assert_eq!(count, 2);
    let transaction_count_response = server
        .get("/api/gateway_wallet/count?status=Failed")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;
    transaction_count_response.assert_status_success();
    let count = transaction_count_response.json::<u64>();
    assert_eq!(count, 0);
    let transaction_count_response = server
        .get("/api/gateway_wallet/count")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token1))
        .await;
    transaction_count_response.assert_status_success();
    let count = transaction_count_response.json::<u64>();
    assert_eq!(count, 2);
    let transaction_count_response = server
        .get("/api/gateway_wallet/count")
        .add_header("Accept", "application/json")
        .add_header("Authorization", format!("Bearer {}", token))
        .await;
    transaction_count_response.assert_status_success();
    let count = transaction_count_response.json::<u64>();
    assert_eq!(count, 3);
});
