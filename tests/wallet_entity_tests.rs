mod helpers;
use chrono::DateTime;
use darve_server::entities::wallet::balance_transaction_entity::TransactionType;
use darve_server::entities::wallet::gateway_transaction_entity::GatewayTransaction;
use darve_server::entities::wallet::lock_transaction_entity::{
    LockTransactionDbService, UnlockTrigger,
};
use darve_server::entities::wallet::wallet_entity::{CurrencySymbol, WalletBalancesView};
use darve_server::middleware;
use darve_server::middleware::ctx::Ctx;
use darve_server::models::view::balance_tx::CurrencyTransactionView;
use futures::future::join_all;
use helpers::{create_fake_login_test_user, create_login_test_user};
use middleware::utils::string_utils::get_string_thing;
use std::time::SystemTime;

test_with_server!(test_wallet_history, |server, ctx_state, config| {
    // create 2 users with user1 and user2 names
    let username1 = "userrr1".to_string();
    let username2 = "userrr2".to_string();

    let (server, user_ident1) = create_login_test_user(&server, username1.clone()).await;

    let (server, user_ident2) = create_login_test_user(&server, username2.clone()).await;

    let _ = get_string_thing(user_ident1.clone()).expect("user1");
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

    let endow_response_text = endow_user_response.text();
    println!("endow_user_response: {}", endow_response_text);
    endow_user_response.assert_status_success();

    // check transaction history /api/user/wallet/history
    let transaction_history_response = server
        .get("/api/wallet/history")
        .add_header("Accept", "application/json")
        .await;

    transaction_history_response.assert_status_success();

    let transactions = &transaction_history_response.json::<Vec<CurrencyTransactionView>>();
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions.get(0).unwrap().amount_in, Some(endow_amt));
});

test_with_server!(lock_user_balance, |server, ctx_state, config| {
    println!("Creating test server");
    let (server, ..) = create_fake_login_test_user(&server).await;
    let (server, user2, ..) = create_fake_login_test_user(&server).await;

    // endow using user2 by calling /api/dev/endow/:user_id/:amount
    let endow_amt = 32;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user2.id.as_ref().unwrap().to_raw(),
            endow_amt
        ))
        .add_header("Accept", "application/json")
        .json("")
        .await;

    endow_user_response.assert_status_success();

    let ctx = Ctx::new(Ok(user2.id.as_ref().unwrap().to_raw()), false);
    let lock_amt = 32;
    let transaction_service = LockTransactionDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };
    let lock_id = transaction_service
        .lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            lock_amt,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
            None,
            TransactionType::Donate,
        )
        .await
        .unwrap();

    let response = server
        .get("/api/wallet/balance")
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let balances = response.json::<WalletBalancesView>();
    dbg!(&balances);
    assert_eq!(balances.balance_locked.balance_usd, lock_amt);
    assert_eq!(balances.balance.balance_usd, endow_amt - lock_amt);
    let _unlock_id = LockTransactionDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .unlock_user_asset_tx(&lock_id, None, TransactionType::Donate)
    .await
    .unwrap();

    let response = server
        .get("/api/wallet/balance")
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let balances = response.json::<WalletBalancesView>(); //dbg!(&balances);
    assert_eq!(balances.balance_locked.balance_usd, 0);
    assert_eq!(balances.balance.balance_usd, endow_amt);
});

test_with_server!(check_balance_too_low, |server, ctx_state, config| {
    let (server, ..) = create_fake_login_test_user(&server).await;
    let (server, user2, ..) = create_fake_login_test_user(&server).await;

    // endow using user2 by calling /api/dev/endow/:user_id/:amount
    let endow_amt = 32;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user2.id.as_ref().unwrap().to_raw(),
            endow_amt
        ))
        .add_header("Accept", "application/json")
        .json("")
        .await;

    let endow_response_text = endow_user_response.text();
    println!("endow_user_response: {}", endow_response_text);
    endow_user_response.assert_status_success();

    let ctx = Ctx::new(Ok(user2.id.as_ref().unwrap().to_raw()), false);
    let transaction_service = LockTransactionDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };
    let res_1 = transaction_service
        .lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            100,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
            None,
            TransactionType::Donate,
        )
        .await;

    assert_eq!(
        res_1.as_ref().err().unwrap().error,
        middleware::error::AppError::BalanceTooLow
    );

    let res_2 = transaction_service
        .lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            100,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
            None,
            TransactionType::Donate,
        )
        .await;
    assert_eq!(
        res_2.as_ref().err().unwrap().error,
        middleware::error::AppError::BalanceTooLow
    );

    let res_3 = transaction_service
        .lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            31,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
            None,
            TransactionType::Donate,
        )
        .await;
    assert!(res_3.as_ref().is_ok());

    let res_4 = transaction_service
        .lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            1,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
            None,
            TransactionType::Donate,
        )
        .await;
    assert!(res_4.as_ref().is_ok());

    let res_5 = transaction_service
        .lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            1,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
            None,
            TransactionType::Donate,
        )
        .await;
    assert_eq!(
        res_5.as_ref().err().unwrap().error,
        middleware::error::AppError::BalanceTooLow
    );
});

test_with_server!(
    check_lock_user_wallet_parallel_1,
    |server, ctx_state, config| {
        let (server, user2, _, token2) = create_fake_login_test_user(&server).await;
        let endow_amt = 30;
        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user2.id.as_ref().unwrap().to_raw(),
                endow_amt
            ))
            .add_header("Accept", "application/json")
            .json("")
            .await;

        endow_user_response.assert_status_success();
        let endow_response_text = endow_user_response.text();
        println!("endow_user_response: {}", endow_response_text);
        let ctx = Ctx::new(Ok(user2.id.as_ref().unwrap().to_raw()), false);
        let transaction_service = LockTransactionDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        };

        let lock_amt = 5;
        let res0 = transaction_service.lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            lock_amt,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
            None,
            TransactionType::Donate,
        );

        let res1 = transaction_service.lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            lock_amt,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
            None,
            TransactionType::Donate,
        );

        let res = join_all([res0, res1]).await;
        assert!(res[0].is_ok());
        assert!(res[1].is_err());

        let res = transaction_service
            .lock_user_asset_tx(
                &user2.id.as_ref().unwrap(),
                1,
                CurrencySymbol::USD,
                vec![UnlockTrigger::Timestamp {
                    at: DateTime::from(SystemTime::now()),
                }],
                None,
                TransactionType::Donate,
            )
            .await;
        assert!(res.is_ok());
        let res = server
            .get("/api/wallet/balance")
            .add_header("Cookie", format!("jwt={}", token2))
            .await;
        res.assert_status_success();
        let bal: WalletBalancesView = res.json::<WalletBalancesView>();
        assert_eq!(bal.balance_locked.balance_usd, 6);
        assert_eq!(bal.balance.balance_usd, 24);
        println!("bal: {}", serde_json::to_string_pretty(&bal).unwrap());
    }
);

test_with_server!(
    check_lock_user_wallet_parallel_2,
    |server, ctx_state, config| {
        let (server, user2, ..) = create_fake_login_test_user(&server).await;
        let endow_amt = 32;
        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user2.id.as_ref().unwrap().to_raw(),
                endow_amt
            ))
            .add_header("Accept", "application/json")
            .json("")
            .await;

        endow_user_response.assert_status_success();
        let endow_response_text = endow_user_response.text();
        println!("endow_user_response: {}", endow_response_text);
        let ctx = Ctx::new(Ok(user2.id.as_ref().unwrap().to_raw()), false);
        let transaction_service = LockTransactionDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        };

        let res0 = transaction_service.lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            1,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
            None,
            TransactionType::Donate,
        );
        let res1 = transaction_service.lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            1,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
            None,
            TransactionType::Donate,
        );
        let res = join_all([res0, res1]).await;
        assert!(res[0].is_ok());
        assert!(res[1].is_err());
    }
);

test_with_server!(prod_balance_0, |server, ctx_state, config| {
    let (_, _user, _password, _) = create_fake_login_test_user(&server).await;
    let response = server
        .get("/api/wallet/balance")
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
            "/test/api/endow/{}/{}",
            user.id.as_ref().unwrap().to_raw(),
            endow_amt
        ))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    endow_user_response.assert_status_success();

    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user.id.as_ref().unwrap().to_raw(),
            endow_amt
        ))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    endow_user_response.assert_status_success();

    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user.id.as_ref().unwrap().to_raw(),
            endow_amt
        ))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    endow_user_response.assert_status_success();

    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;

    let endow_amt = 32;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user1.id.as_ref().unwrap().to_raw(),
            endow_amt
        ))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    endow_user_response.assert_status_success();

    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user1.id.as_ref().unwrap().to_raw(),
            endow_amt
        ))
        .add_header("Accept", "application/json")
        .json("")
        .await;
    endow_user_response.assert_status_success();

    let transaction_history_response = server
        .get("/api/gateway_wallet/history")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token))
        .await;

    transaction_history_response.assert_status_success();

    let transactions = &transaction_history_response.json::<Vec<GatewayTransaction>>();
    assert_eq!(transactions.len(), 3);

    let transaction_history_response = server
        .get("/api/gateway_wallet/history?type=Deposit")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token))
        .await;

    transaction_history_response.assert_status_success();

    let transactions = &transaction_history_response.json::<Vec<GatewayTransaction>>();
    assert_eq!(transactions.len(), 3);

    let transaction_history_response = server
        .get("/api/gateway_wallet/history?type=Withdraw")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token))
        .await;

    transaction_history_response.assert_status_success();

    let transactions = &transaction_history_response.json::<Vec<GatewayTransaction>>();
    assert_eq!(transactions.len(), 0);

    let transaction_history_response = server
        .get("/api/gateway_wallet/history")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    transaction_history_response.assert_status_success();

    let transactions = &transaction_history_response.json::<Vec<GatewayTransaction>>();
    assert_eq!(transactions.len(), 2);

    let transaction_history_response = server
        .get("/api/gateway_wallet/history?status=Failed")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    transaction_history_response.assert_status_success();

    let transactions = &transaction_history_response.json::<Vec<GatewayTransaction>>();
    assert_eq!(transactions.len(), 0);

    let transaction_history_response = server
        .get("/api/gateway_wallet/history?status=Completed")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    transaction_history_response.assert_status_success();

    let transactions = &transaction_history_response.json::<Vec<GatewayTransaction>>();
    assert_eq!(transactions.len(), 2);
    let transaction_count_response = server
        .get("/api/gateway_wallet/count?status=Completed")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;
    transaction_count_response.assert_status_success();
    let count = transaction_count_response.json::<u64>();
    assert_eq!(count, 2);
    let transaction_count_response = server
        .get("/api/gateway_wallet/count?status=Failed")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;
    transaction_count_response.assert_status_success();
    let count = transaction_count_response.json::<u64>();
    assert_eq!(count, 0);
    let transaction_count_response = server
        .get("/api/gateway_wallet/count")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;
    transaction_count_response.assert_status_success();
    let count = transaction_count_response.json::<u64>();
    assert_eq!(count, 2);
    let transaction_count_response = server
        .get("/api/gateway_wallet/count")
        .add_header("Accept", "application/json")
        .add_header("Cookie", format!("jwt={}", token))
        .await;
    transaction_count_response.assert_status_success();
    let count = transaction_count_response.json::<u64>();
    assert_eq!(count, 3);
});
