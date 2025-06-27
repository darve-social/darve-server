mod helpers;

use axum::extract::State;
use chrono::DateTime;
use darve_server::entities::wallet::lock_transaction_entity::{
    LockTransactionDbService, UnlockTrigger,
};
use darve_server::entities::wallet::wallet_entity::{CurrencySymbol, WalletBalancesView};
use darve_server::middleware::ctx::Ctx;
use darve_server::routes::wallet::wallet_routes::get_user_balance;
use darve_server::{middleware, routes::wallet::wallet_routes};
use futures::future::join_all;
use helpers::{create_fake_login_test_user, create_login_test_user, create_test_server};
use middleware::utils::string_utils::get_string_thing;
use serial_test::serial;
use std::time::SystemTime;
use uuid::Uuid;
use wallet_routes::CurrencyTransactionHistoryView;

#[tokio::test]
#[serial]
async fn test_wallet_history() {
    // create test server
    println!("Creating test server");
    let (server, _ctx_state) = create_test_server().await;

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
        .get("/api/user/wallet/history")
        .add_header("Accept", "application/json")
        .await;

    transaction_history_response.assert_status_success();

    let created = &transaction_history_response.json::<CurrencyTransactionHistoryView>();
    assert_eq!(created.transactions.len(), 1);
    assert_eq!(
        created.transactions.get(0).unwrap().amount_in,
        Some(endow_amt)
    );
}

#[tokio::test]
#[serial]
async fn lock_user_balance() {
    println!("Creating test server");
    let (server, ctx_state) = create_test_server().await;

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

    let ctx = Ctx::new(
        Ok(user2.id.as_ref().unwrap().to_raw()),
        Uuid::new_v4(),
        false,
    );
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
        )
        .await
        .unwrap();

    let response = server
        .get("/api/user/wallet/balance")
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
    .unlock_user_asset_tx(&lock_id)
    .await
    .unwrap();

    let response = server
        .get("/api/user/wallet/balance")
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let balances = response.json::<WalletBalancesView>(); //dbg!(&balances);
    assert_eq!(balances.balance_locked.balance_usd, 0);
    assert_eq!(balances.balance.balance_usd, endow_amt);
}

#[tokio::test]
#[serial]
async fn check_balance_too_low() {
    let (server, ctx_state) = create_test_server().await;
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

    let ctx = Ctx::new(
        Ok(user2.id.as_ref().unwrap().to_raw()),
        Uuid::new_v4(),
        false,
    );
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
        )
        .await;
    assert_eq!(
        res_5.as_ref().err().unwrap().error,
        middleware::error::AppError::BalanceTooLow
    );
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn check_lock_user_wallet_parallel_1() {
    println!("Creating test server");
    let (server, ctx_state) = create_test_server().await;
    let (server, user2, ..) = create_fake_login_test_user(&server).await;
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
    let ctx = Ctx::new(
        Ok(user2.id.as_ref().unwrap().to_raw()),
        Uuid::new_v4(),
        false,
    );
    let transaction_service = LockTransactionDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };

    let lock_amt = 5;
    let res = join_all([
        transaction_service.lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            lock_amt,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
        ),
        transaction_service.lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            lock_amt,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
        ),
        transaction_service.lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            lock_amt,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
        ),
    ])
    .await;

    assert!(res[0].is_ok());
    assert!(res[1].is_ok());
    assert!(res[2].is_ok());

    let res = transaction_service
        .lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            1,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
        )
        .await;
    assert!(res.is_ok());

    let bal = get_user_balance(ctx, State(ctx_state)).await;
    let bal: WalletBalancesView = serde_json::from_str(bal.unwrap().0.as_str()).unwrap();
    assert_eq!(bal.balance_locked.balance_usd, 16);
    assert_eq!(bal.balance.balance_usd, 14);
    println!("bal: {}", serde_json::to_string_pretty(&bal).unwrap());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn check_lock_user_wallet_parallel_2() {
    println!("Creating test server");
    let (server, ctx_state) = create_test_server().await;
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
    let ctx = Ctx::new(
        Ok(user2.id.as_ref().unwrap().to_raw()),
        Uuid::new_v4(),
        false,
    );
    let transaction_service = LockTransactionDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };

    let res = join_all([
        transaction_service.lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            1,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
        ),
        transaction_service.lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            1,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
        ),
        transaction_service.lock_user_asset_tx(
            &user2.id.as_ref().unwrap(),
            1,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Timestamp {
                at: DateTime::from(SystemTime::now()),
            }],
        ),
    ])
    .await;

    assert!(res[0].is_ok());
    assert!(res[1].is_ok());
    assert!(res[2].is_ok());
}
#[tokio::test]
#[serial]
async fn prod_balance_0() {
    let (server, _state) = create_test_server().await;
    let (_, _user, _password, _) = create_fake_login_test_user(&server).await;
    let response = server
        .get("/api/user/wallet/balance")
        .add_header("Accept", "application/json")
        .await;
    response.assert_status_success();
    let balances = response.json::<WalletBalancesView>();
    assert_eq!(balances.balance_locked.balance_usd, 0);
}
