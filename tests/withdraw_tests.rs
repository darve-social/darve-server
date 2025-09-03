mod helpers;
use darve_server::entities::wallet::balance_transaction_entity::{
    CurrencyTransaction, TransactionType,
};
use darve_server::entities::wallet::gateway_transaction_entity::{
    GatewayTransactionDbService, GatewayTransactionStatus,
};
use darve_server::entities::wallet::wallet_entity::{
    WalletDbService, APP_GATEWAY_WALLET, DARVE_WALLET,
};
use darve_server::middleware::ctx::Ctx;
use darve_server::middleware::error::AppError;
use surrealdb::sql::Thing;

use crate::helpers::create_fake_login_test_user;

test_with_server!(
    try_to_withdraw_more_than_balance,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;

        let endow_user_response = server
            .get(&format!(
                "/test/api/endow/{}/{}",
                user.id.as_ref().unwrap().to_raw(),
                100000
            ))
            .add_header("Accept", "application/json")
            .json("")
            .await;

        let endow_response_text = endow_user_response.text();
        println!("endow_user_response: {}", endow_response_text);
        endow_user_response.assert_status_success();

        let gateway_db_service = GatewayTransactionDbService {
            db: &ctx_state.db.client,
            ctx: &Ctx::new(Ok(user.id.as_ref().unwrap().to_raw()), false),
        };

        let res = gateway_db_service
            .user_withdraw_tx_start(
                user.id.as_ref().unwrap(),
                200000,
                None,
                ctx_state.withdraw_fee,
            )
            .await;
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().error, AppError::BalanceTooLow);
    }
);

test_with_server!(withdraw_complete, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let amount: u64 = 100000;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user.id.as_ref().unwrap().to_raw(),
            amount
        ))
        .add_header("Accept", "application/json")
        .await;

    let endow_response_text = endow_user_response.text();
    println!("endow_user_response: {}", endow_response_text);
    endow_user_response.assert_status_success();

    let gateway_db_service = GatewayTransactionDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok(user.id.as_ref().unwrap().to_raw()), false),
    };

    let res = gateway_db_service
        .user_withdraw_tx_start(
            user.id.as_ref().unwrap(),
            amount,
            None,
            ctx_state.withdraw_fee,
        )
        .await;
    assert!(res.is_ok());

    let gateway_id = res.unwrap();

    let res = gateway_db_service
        .user_withdraw_tx_complete(gateway_id.clone())
        .await;

    assert!(res.is_ok());

    let txs = gateway_db_service
        .get_by_user(
            user.id.as_ref().unwrap(),
            None,
            Some(TransactionType::Withdraw),
            None,
        )
        .await
        .unwrap();

    assert_eq!(txs.len(), 1);
    let tx = txs.first().unwrap();
    assert_eq!(
        tx.status,
        Some(GatewayTransactionStatus::Completed.to_string())
    );
    assert!(tx.fee_tx.is_some());
    let fee = (amount as f64 * ctx_state.withdraw_fee) as u64;
    assert_eq!(fee, 5000);
    let txs = ctx_state
        .db
        .client
        .query("SELECT * FROM balance_transaction WHERE wallet=$wallet AND gateway_tx=$gateway_tx")
        .bind(("wallet", APP_GATEWAY_WALLET.clone()))
        .bind(("gateway_tx", gateway_id.clone()))
        .await
        .unwrap()
        .take::<Vec<CurrencyTransaction>>(0)
        .unwrap();

    assert_eq!(txs.len(), 1);
    let tx = txs.first().unwrap();
    assert_eq!(tx.amount_in, Some((amount - fee) as i64));

    let txs = ctx_state
        .db
        .client
        .query("SELECT * FROM balance_transaction WHERE wallet=$wallet AND gateway_tx=$gateway_tx")
        .bind(("wallet", DARVE_WALLET.clone()))
        .bind(("gateway_tx", gateway_id.clone()))
        .await
        .unwrap()
        .take::<Vec<CurrencyTransaction>>(0)
        .unwrap();

    assert_eq!(txs.len(), 1);
    let tx = txs.first().unwrap();
    assert_eq!(tx.amount_in, Some(fee as i64));
    assert_eq!(tx.r#type, Some(TransactionType::Fee));
});

test_with_server!(withdraw_revert, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let amount: u64 = 100000;
    let endow_user_response = server
        .get(&format!(
            "/test/api/endow/{}/{}",
            user.id.as_ref().unwrap().to_raw(),
            amount
        ))
        .add_header("Accept", "application/json")
        .await;

    let endow_response_text = endow_user_response.text();
    println!("endow_user_response: {}", endow_response_text);
    endow_user_response.assert_status_success();

    let gateway_db_service = GatewayTransactionDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok(user.id.as_ref().unwrap().to_raw()), false),
    };

    let res = gateway_db_service
        .user_withdraw_tx_start(
            user.id.as_ref().unwrap(),
            amount,
            None,
            ctx_state.withdraw_fee,
        )
        .await;
    assert!(res.is_ok());

    let gateway_id = res.unwrap();

    let res = gateway_db_service
        .user_withdraw_tx_revert(gateway_id.clone(), None)
        .await;

    assert!(res.is_ok());

    let txs = gateway_db_service
        .get_by_user(
            user.id.as_ref().unwrap(),
            None,
            Some(TransactionType::Withdraw),
            None,
        )
        .await
        .unwrap();

    assert_eq!(txs.len(), 1);
    let tx = txs.first().unwrap();
    assert_eq!(
        tx.status,
        Some(GatewayTransactionStatus::Failed.to_string())
    );
    assert!(tx.fee_tx.is_none());

    let wallet_service = WalletDbService {
        db: &ctx_state.db.client,
        ctx: &Ctx::new(Ok("".to_string()), false),
    };

    let balance = wallet_service
        .get_balance(&Thing::from((
            "wallet",
            user.id.as_ref().unwrap().id.to_raw().as_str(),
        )))
        .await
        .unwrap();

    assert_eq!(balance.balance_usd, amount as i64);
});
