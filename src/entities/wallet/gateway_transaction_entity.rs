use balance_transaction_entity::BalanceTransactionDbService;
use middleware::db;
use middleware::utils::db_utils::{get_entity, with_not_found_err, IdentIdName};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
use wallet_entity::{CurrencySymbol, WalletDbService, APP_GATEWAY_WALLET};

use crate::entities::user_auth::local_user_entity;
use crate::entities::wallet::lock_transaction_entity::{LockTransactionDbService, UnlockTrigger};
use crate::entities::wallet::wallet_entity::check_transaction_custom_error;
use crate::middleware;

use super::{balance_transaction_entity, lock_transaction_entity, wallet_entity};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GatewayTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub amount: i64,
    pub currency: CurrencySymbol,
    pub external_tx_id: String,
    pub external_account_id: Option<String>,
    pub internal_tx: Option<Thing>,
    pub user: Thing,
    pub withdraw_lock_tx: Option<Thing>,
    pub withdraw_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

enum WithdrawStatus {
    Locked,
    ExternalProcess,
    Complete,
}

pub struct GatewayTransactionDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "gateway_transaction";
const USER_TABLE: &str = local_user_entity::TABLE_NAME;
const TRANSACTION_TABLE: &str = balance_transaction_entity::TABLE_NAME;
const LOCK_TRANSACTION_TABLE: &str = lock_transaction_entity::TABLE_NAME;

impl<'a> GatewayTransactionDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let curr_usd = CurrencySymbol::USD;
        let curr_reef = CurrencySymbol::REEF;
        let curr_eth = CurrencySymbol::ETH;

        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS external_tx_id ON TABLE {TABLE_NAME} TYPE string VALUE $before OR $value;
    DEFINE FIELD IF NOT EXISTS external_account_id ON TABLE {TABLE_NAME} TYPE string VALUE $before OR $value;
    DEFINE FIELD IF NOT EXISTS internal_tx ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>> VALUE $before OR $value;
    DEFINE FIELD IF NOT EXISTS withdraw_lock_tx ON TABLE {TABLE_NAME} TYPE option<record<{LOCK_TRANSACTION_TABLE}>> VALUE $before OR $value;
    DEFINE FIELD IF NOT EXISTS withdraw_status ON TABLE {TABLE_NAME} TYPE option<string> ASSERT $value INSIDE ['LOCKED','EXTERNAL_PROCESS','COMPLETE'] ;
    DEFINE FIELD IF NOT EXISTS user ON TABLE {TABLE_NAME} TYPE record<{USER_TABLE}>;
    DEFINE INDEX IF NOT EXISTS user_idx ON TABLE {TABLE_NAME} COLUMNS user;
    DEFINE FIELD IF NOT EXISTS amount ON TABLE {TABLE_NAME} TYPE number;
    DEFINE FIELD IF NOT EXISTS currency ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0
        ASSERT $value INSIDE ['{curr_usd}','{curr_reef}','{curr_eth}'];
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();

    ");
        let mutation = self.db.query(sql).await?;

        mutation.check().expect("should mutate gatewayTransaction");

        Ok(())
    }

    // creates gatewayTransaction
    pub(crate) async fn user_deposit_tx(
        &self,
        user: &Thing,
        external_account: String,
        external_tx_id: String,
        amount: i64,
        currency_symbol: CurrencySymbol,
    ) -> CtxResult<Thing> {
        let user_wallet = WalletDbService::get_user_wallet_id(user);

        let gwy_wallet = APP_GATEWAY_WALLET.clone();
        let fund_tx_id = Thing::from((TABLE_NAME, Id::ulid()));

        let gateway_2_user_tx = BalanceTransactionDbService::get_transfer_qry(
            &gwy_wallet,
            &user_wallet,
            amount,
            &currency_symbol,
            Some(fund_tx_id.clone()),
            None,
            true,
        )?;
        let gateway_2_user_qry = gateway_2_user_tx.get_query_string();

        let fund_qry = format!(
            "
        BEGIN TRANSACTION;

            LET $fund_tx = INSERT INTO {TABLE_NAME} {{
                id: $fund_tx_id,
                amount: $fund_amt,
                user: $user,
                external_tx_id: $ext_tx,
                external_account_id:$ext_account_id,
                currency: $currency,
            }} RETURN id;

            //LET $fund_id = $fund_tx[0].id;

           {gateway_2_user_qry}

            RETURN $fund_tx[0].id;
        COMMIT TRANSACTION;

        "
        );
        let qry = self
            .db
            .query(fund_qry)
            .bind(("fund_tx_id", fund_tx_id))
            .bind(("fund_amt", amount))
            .bind(("user", user.clone()))
            .bind(("ext_tx", external_tx_id))
            .bind(("ext_account_id", external_account))
            .bind(("currency", currency_symbol));

        let qry = gateway_2_user_tx
            .get_bindings()
            .iter()
            .fold(qry, |q, item| q.bind((item.0.clone(), item.1.clone())));

        let mut fund_res = qry.await?;
        fund_res = fund_res.check()?;
        let res: Option<Thing> = fund_res.take(0)?;
        res.ok_or(self.ctx.to_ctx_error(AppError::Generic {
            description: "Error in endowment tx".to_string(),
        }))
    }

    pub(crate) async fn user_withdraw_tx_start(
        &self,
        user: &Thing,
        amount: i64,
        external_account_id: String,
    ) -> CtxResult<Thing> {
        let withdraw_fund_tx_id = Thing::from((TABLE_NAME, Id::ulid()));

        let lock_db_service = LockTransactionDbService {
            db: self.db,
            ctx: self.ctx,
        };

        let user_2_lock_qry_bindings = lock_db_service.lock_user_asset_qry(
            user,
            amount,
            CurrencySymbol::USD,
            vec![UnlockTrigger::Withdraw {
                id: withdraw_fund_tx_id.clone(),
            }],
            true,
        )?;
        let user_2_lock_qry = user_2_lock_qry_bindings.get_query_string();
        let qry = format!(
            "\
         BEGIN TRANSACTION;

           {user_2_lock_qry}

            LET $fund_tx = INSERT INTO {TABLE_NAME} {{
                id: $fund_tx_id,
                amount: $fund_amt,
                user: $user,
                withdraw_status: 'LOCKED',
                withdraw_lock_tx: $lock_tx_id,
                external_account_id:$ext_account_id,
                currency: $currency,
            }} RETURN id;

            LET $fund_tx_id = $fund_tx[0].id;
            $fund_tx_id;
        COMMIT TRANSACTION;"
        );

        let qry = self
            .db
            .query(qry)
            .bind(("fund_tx_id", withdraw_fund_tx_id))
            .bind(("fund_amt", amount))
            .bind(("user", user.clone()))
            .bind(("ext_account_id", external_account_id))
            .bind(("currency", CurrencySymbol::USD));

        let qry = user_2_lock_qry_bindings
            .get_bindings()
            .into_iter()
            .fold(qry, |q, item| q.bind((item.0, item.1)));

        let mut fund_res = qry.await?;
        check_transaction_custom_error(&mut fund_res)?;

        let res: Option<Thing> = fund_res.take(22)?;
        res.ok_or(self.ctx.to_ctx_error(AppError::Generic {
            description: "Error in withdraw tx".to_string(),
        }))
    }

    pub(crate) async fn user_withdraw_tx_revert(
        &self,
        withdraw_tx_id: Thing,
        external_tx_id: String,
    ) -> CtxResult<()> {
        let withdraw_tx = self.get(IdentIdName::Id(withdraw_tx_id)).await?;

        // TODO check if external tx matches, amount matches
        let lock_db_service = LockTransactionDbService {
            db: self.db,
            ctx: self.ctx,
        };

        let lock_tx_id =
            withdraw_tx
                .withdraw_lock_tx
                .ok_or(self.ctx.to_ctx_error(AppError::Generic {
                    description: "Lock tx not found".to_string(),
                }))?;
        lock_db_service.unlock_user_asset_tx(&lock_tx_id).await?;
        Ok(())
    }

    pub(crate) async fn user_withdraw_tx_complete(
        &self,
        withdraw_tx_id: Thing,
        external_tx_id: String,
    ) -> CtxResult<()> {
        let withdraw_tx = self.get(IdentIdName::Id(withdraw_tx_id)).await?;

        // TODO check if external tx matches, amount matches
        let lock_db_service = LockTransactionDbService {
            db: self.db,
            ctx: self.ctx,
        };

        let lock_tx_id =
            withdraw_tx
                .withdraw_lock_tx
                .ok_or(self.ctx.to_ctx_error(AppError::Generic {
                    description: "Lock tx not found".to_string(),
                }))?;
        lock_db_service
            .process_locked_payment(&lock_tx_id, &APP_GATEWAY_WALLET.clone())
            .await?;
        Ok(())
    }

    pub(crate) async fn user_withdraw_tx_status_update(
        &self,
        withdraw_tx_id: Thing,
        external_tx_id: String,
        new_status: String,
    ) -> CtxResult<()> {
        todo!()
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<GatewayTransaction> {
        let opt =
            get_entity::<GatewayTransaction>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

    pub fn unknown_endowment_user_id(&self) -> Thing {
        Thing::try_from((USER_TABLE, "unrecognised_user_endowment_id")).expect("is valid")
    }
}
