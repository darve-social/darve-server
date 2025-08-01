use super::{gateway_transaction_entity, lock_transaction_entity, wallet_entity};
use crate::database::client::Db;
use crate::entities::wallet::wallet_entity::check_transaction_custom_error;
use crate::middleware;
use crate::middleware::error::CtxError;
use crate::routes::wallet::CurrencyTransactionView;
use middleware::error::AppResult;
use middleware::utils::db_utils::{
    get_entity, get_entity_list_view, with_not_found_err, IdentIdName, Pagination, QryBindingsVal,
};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::sql::{to_value, Thing, Value};
use wallet_entity::{CurrencySymbol, WalletDbService, APP_GATEWAY_WALLET};

#[derive(Debug, Deserialize)]
pub struct TransferCurrencyResponse {
    pub tx_in_id: Thing,
    pub tx_out_id: Thing,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurrencyTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub wallet: Thing,
    pub with_wallet: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transfer_title: Option<String>,
    pub tx_ident: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_tx: Option<Thing>,
    pub currency: CurrencySymbol,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_in: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_out: Option<i64>,
    pub balance: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

pub struct BalanceTransactionDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "balance_transaction";
const WALLET_TABLE: &str = wallet_entity::TABLE_NAME;
const GATEWAY_TX_TABLE: &str = gateway_transaction_entity::TABLE_NAME;
const LOCK_TX_TABLE: &str = lock_transaction_entity::TABLE_NAME;

pub const THROW_BALANCE_TOO_LOW: &str = "Not enough balance";

impl<'a> BalanceTransactionDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let _gateway_wallet = APP_GATEWAY_WALLET.clone();
        let curr_usd = CurrencySymbol::USD.to_string();
        let curr_reef = CurrencySymbol::REEF.to_string();
        let curr_eth = CurrencySymbol::ETH.to_string();
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS wallet ON TABLE {TABLE_NAME} TYPE record<{WALLET_TABLE}>;
    DEFINE FIELD IF NOT EXISTS currency ON TABLE {TABLE_NAME} TYPE string ASSERT $value INSIDE ['{curr_usd}','{curr_reef}','{curr_eth}'];
    DEFINE INDEX IF NOT EXISTS wallet_currency_idx ON {TABLE_NAME} FIELDS wallet, currency;
    DEFINE INDEX IF NOT EXISTS wallet_idx ON {TABLE_NAME} FIELDS wallet;
    DEFINE FIELD IF NOT EXISTS with_wallet ON TABLE {TABLE_NAME} TYPE record<{WALLET_TABLE}>;
    DEFINE FIELD IF NOT EXISTS transfer_title ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS tx_ident ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD IF NOT EXISTS lock_tx ON TABLE {TABLE_NAME} TYPE option<record<{LOCK_TX_TABLE}>>;
    DEFINE FIELD IF NOT EXISTS gateway_tx ON TABLE {TABLE_NAME} TYPE option<record<{GATEWAY_TX_TABLE}>>;
    DEFINE FIELD IF NOT EXISTS prev_transaction ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_NAME}>>;
    DEFINE FIELD IF NOT EXISTS amount_in ON TABLE {TABLE_NAME} TYPE option<number>;
    DEFINE FIELD IF NOT EXISTS amount_out ON TABLE {TABLE_NAME} TYPE option<number>;
    DEFINE FIELD IF NOT EXISTS balance ON TABLE {TABLE_NAME} TYPE number;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    DEFINE INDEX IF NOT EXISTS r_created_idx ON {TABLE_NAME} FIELDS r_created;
    ");
        let mutation = self.db.query(sql).await?;

        mutation.check().expect("should mutate currencyTransaction");
        let g_wallet = WalletDbService {
            db: self.db,
            ctx: self.ctx,
        }
        .init_app_gateway_wallet()
        .await;
        if let Err(err) = g_wallet {
            if !err.error.to_string().contains("Wallet already exists") {
                return Err(err.error);
            }
        }
        Ok(())
    }

    pub async fn transfer_task_reward(
        &self,
        wallet_from: &Thing,
        wallet_to: &Thing,
        amount: i64,
        currency: &CurrencySymbol,
        task_user_id: &Thing,
    ) -> CtxResult<()> {
        let tx_qry =
            Self::get_transfer_qry(wallet_from, wallet_to, amount, currency, None, None, true)?;

        let mut query = self
            .db
            .query("BEGIN TRANSACTION")
            .query(tx_qry.get_query_string())
            .query("UPDATE $task_user_id SET reward_tx=$tx_in_id")
            .query("COMMIT TRANSACTION")
            .bind(("task_user_id", task_user_id.clone()));

        for v in tx_qry.get_bindings().into_iter() {
            query = query.bind(v);
        }
        let _ = query.await?.check()?;
        Ok(())
    }

    pub async fn transfer_currency(
        &self,
        wallet_from: &Thing,
        wallet_to: &Thing,
        amount: i64,
        currency: &CurrencySymbol,
    ) -> CtxResult<TransferCurrencyResponse> {
        let tx_qry =
            Self::get_transfer_qry(wallet_from, wallet_to, amount, currency, None, None, false)?;
        let mut query = self
            .db
            .query(tx_qry.get_query_string())
            .query("RETURN {{ tx_in_id: $tx_in_id, tx_out_id: $tx_out_id }}");

        for v in tx_qry.get_bindings().into_iter() {
            query = query.bind(v);
        }

        let mut res = query.await?;
        check_transaction_custom_error(&mut res)?;
        let index = res.num_statements() - 1;
        Ok(res
            .take::<Option<TransferCurrencyResponse>>(index)?
            .unwrap())
    }

    pub async fn user_transaction_list(
        &self,
        wallet_id: &Thing,
        pagination: Option<Pagination>,
    ) -> CtxResult<Vec<CurrencyTransactionView>> {
        WalletDbService::is_wallet_id(self.ctx.clone(), wallet_id)?;
        get_entity_list_view::<CurrencyTransactionView>(
            self.db,
            TABLE_NAME.to_string(),
            &IdentIdName::ColumnIdent {
                column: "wallet".to_string(),
                val: wallet_id.to_raw(),
                rec: true,
            },
            pagination,
        )
        .await
    }

    pub(crate) async fn create_init_record(
        &self,
        wallet_id: &Thing,
        currency: CurrencySymbol,
    ) -> CtxResult<CurrencyTransaction> {
        let record = CurrencyTransaction {
            id: None,
            wallet: wallet_id.clone(),
            with_wallet: Thing::from((WALLET_TABLE, "init_wallet")),
            transfer_title: None,
            tx_ident: wallet_id.id.to_raw(),
            gateway_tx: None,
            currency,
            amount_in: None,
            amount_out: None,
            balance: 0,
            r_created: None,
            r_updated: None,
        };
        self.db
            .create(TABLE_NAME)
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<CurrencyTransaction>| v.unwrap())
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<CurrencyTransaction> {
        let opt =
            get_entity::<CurrencyTransaction>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

    pub(crate) fn get_transfer_qry(
        wallet_from: &Thing,
        wallet_to: &Thing,
        amount: i64,
        currency: &CurrencySymbol,
        gateway_tx: Option<Thing>,
        lock_tx: Option<Thing>,
        exclude_sql_transaction: bool,
    ) -> AppResult<QryBindingsVal<Value>> {
        let (begin_tx, commit_tx) = if exclude_sql_transaction {
            ("", "")
        } else {
            ("BEGIN TRANSACTION;", "COMMIT TRANSACTION;")
        };

        let qry = format!(
            "{begin_tx}
            LET $lock_id=time::now() + 10s;
            UPDATE $w_from_id SET lock_id=$lock_id;
            LET $upd_lck = UPDATE $w_to_id SET lock_id=$lock_id;
            IF array::len($upd_lck)==0 {{
               CREATE $w_to_id SET lock_id=$lock_id, transaction_head={{}}; 
             }};            
            LET $w_from = SELECT * FROM ONLY $w_from_id FETCH transaction_head[$currency];
            LET $balance = $w_from.transaction_head[$currency].balance OR 0;
            LET $tx_amt = type::number($amt);
            LET $updated_from_balance = $balance - $tx_amt;

            IF $w_from_id!=$app_gateway_wallet_id && $updated_from_balance < 0 {{
                THROW \"{THROW_BALANCE_TOO_LOW}\";                
            }};

            LET $tx_ident = rand::ulid();
            LET $tx_out = INSERT INTO {TABLE_NAME} {{
                id: rand::ulid(),
                wallet: $w_from_id,
                with_wallet:$w_to_id,
                tx_ident: $tx_ident,
                currency: $currency,
                amount_out: $tx_amt,
                balance: $updated_from_balance,
                gateway_tx: $gateway_tx_id,
                lock_tx: $lock_tx_id,
            }} RETURN id;
            LET $tx_out_id = $tx_out[0].id;
            UPDATE $w_from_id SET transaction_head[$currency]=$tx_out_id, lock_id=NONE;
            LET $w_to = SELECT * FROM ONLY $w_to_id FETCH transaction_head[$currency];
            LET $balance_to = $w_to.transaction_head[$currency].balance OR 0;
            LET $updated_to_balance = $balance_to + $tx_amt;
            LET $tx_in = INSERT INTO {TABLE_NAME} {{
                id: rand::ulid(),
                wallet: $w_to_id,
                with_wallet:$w_from_id,
                tx_ident: $tx_ident,
                currency: $currency,
                amount_in: $tx_amt,
                balance: $updated_to_balance,
                gateway_tx: $gateway_tx_id,
                lock_tx: $lock_tx_id,
            }} RETURN id;
            LET $tx_in_id = $tx_in[0].id;
            UPDATE $w_to_id SET transaction_head[$currency]=$tx_in_id, lock_id=NONE;
            {commit_tx}
         "
        );
        let mut bindings = HashMap::new();
        bindings.insert(
            "w_from_id".to_string(),
            to_value(wallet_from.clone()).map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?,
        );
        bindings.insert(
            "w_to_id".to_string(),
            to_value(wallet_to.clone()).map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?,
        );
        bindings.insert(
            "amt".to_string(),
            to_value(amount).map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?,
        );
        bindings.insert(
            "currency".to_string(),
            to_value(currency.clone()).map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?,
        );
        bindings.insert(
            "app_gateway_wallet_id".to_string(),
            to_value(APP_GATEWAY_WALLET.clone()).map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?,
        );
        bindings.insert(
            "gateway_tx_id".to_string(),
            to_value(gateway_tx).map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?,
        );
        bindings.insert(
            "lock_tx_id".to_string(),
            to_value(lock_tx).map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?,
        );
        Ok(QryBindingsVal::new(qry, bindings))
    }
}
