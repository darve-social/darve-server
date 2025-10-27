use std::fmt::Display;

use super::{gateway_transaction_entity, wallet_entity};
use crate::database::client::Db;
use crate::entities::wallet::wallet_entity::check_transaction_custom_error;
use crate::middleware;
use crate::middleware::error::CtxError;
use crate::models::view::balance_tx::CurrencyTransactionView;
use chrono::{DateTime, Utc};
use middleware::utils::db_utils::{
    get_entity, get_entity_list_view, with_not_found_err, IdentIdName, Pagination,
};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};
use serde::{Deserialize, Serialize};
use surrealdb::method::Query;
use surrealdb::sql::Thing;
use wallet_entity::{CurrencySymbol, WalletDbService, APP_GATEWAY_WALLET};

#[derive(Debug, Deserialize)]
pub struct TransferCurrencyResponse {
    pub tx_in_id: Thing,
    pub tx_out_id: Thing,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CurrencyTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub wallet: Thing,
    pub with_wallet: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_tx: Option<Thing>,
    pub currency: CurrencySymbol,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_in: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_out: Option<i64>,
    pub balance: i64,
    pub created_at: DateTime<Utc>,
    pub description: Option<String>,
    pub r#type: Option<TransactionType>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum TransactionType {
    Withdraw,
    Deposit,
    Refund,
    Donate,
    Reward,
    Fee,
}

impl Display for TransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactionType::Withdraw => write!(f, "Withdraw"),
            TransactionType::Deposit => write!(f, "Deposit"),
            TransactionType::Refund => write!(f, "Refund"),
            TransactionType::Donate => write!(f, "Donate"),
            TransactionType::Reward => write!(f, "Reward"),
            TransactionType::Fee => write!(f, "Fee"),
        }
    }
}

pub struct BalanceTransactionDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "balance_transaction";
const WALLET_TABLE: &str = wallet_entity::TABLE_NAME;
const GATEWAY_TX_TABLE: &str = gateway_transaction_entity::TABLE_NAME;

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
    DEFINE FIELD IF NOT EXISTS gateway_tx ON TABLE {TABLE_NAME} TYPE option<record<{GATEWAY_TX_TABLE}>>;
    DEFINE FIELD IF NOT EXISTS prev_transaction ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_NAME}>>;
    DEFINE FIELD IF NOT EXISTS amount_in ON TABLE {TABLE_NAME} TYPE option<number>;
    DEFINE FIELD IF NOT EXISTS amount_out ON TABLE {TABLE_NAME} TYPE option<number>;
    DEFINE FIELD IF NOT EXISTS balance ON TABLE {TABLE_NAME} TYPE number DEFAULT 0;
    DEFINE FIELD IF NOT EXISTS description ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS type ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
    DEFINE INDEX IF NOT EXISTS created_at_idx ON {TABLE_NAME} FIELDS created_at;
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
        description: Option<String>,
    ) -> CtxResult<()> {
        let uniq = "id";
        let query = self.db.query("BEGIN");
        let tx_qry = Self::build_transfer_qry(
            query,
            wallet_from,
            wallet_to,
            amount,
            currency,
            None,
            description,
            TransactionType::Reward,
            uniq,
        )
        .query(format!(
            "UPDATE $task_user_id SET reward_tx=${uniq}_tx_in_id"
        ))
        .query("COMMIT")
        .bind(("task_user_id", task_user_id.clone()));

        let _ = tx_qry.await?.check()?;
        Ok(())
    }

    pub async fn transfer_currency(
        &self,
        wallet_from: &Thing,
        wallet_to: &Thing,
        amount: i64,
        currency: &CurrencySymbol,
        description: Option<String>,
        tx_type: TransactionType,
    ) -> CtxResult<TransferCurrencyResponse> {
        let uniq = "id";
        let query = self.db.query("BEGIN");

        let tx_qry = Self::build_transfer_qry(
            query,
            wallet_from,
            wallet_to,
            amount,
            currency,
            None,
            description,
            tx_type,
            uniq,
        )
        .query(format!(
            "RETURN {{ tx_in_id: ${uniq}_tx_in_id, tx_out_id: ${uniq}_tx_out_id }}"
        ))
        .query("COMMIT");

        let mut res = tx_qry.await?;
        check_transaction_custom_error(&mut res)?;
        let index = res.num_statements() - 1;
        Ok(res
            .take::<Option<TransferCurrencyResponse>>(index)?
            .unwrap())
    }

    pub async fn user_transaction_list(
        &self,
        wallet_id: &Thing,
        tx_type: Option<TransactionType>,
        pagination: Option<Pagination>,
    ) -> CtxResult<Vec<CurrencyTransactionView>> {
        WalletDbService::is_wallet_id(self.ctx.clone(), wallet_id)?;

        let ident = match tx_type {
            Some(ref v) => IdentIdName::ColumnIdentAnd(vec![
                IdentIdName::ColumnIdent {
                    column: "wallet".to_string(),
                    val: wallet_id.to_raw(),
                    rec: true,
                },
                IdentIdName::ColumnIdent {
                    column: "type".to_string(),
                    val: v.to_string(),
                    rec: false,
                },
            ]),
            None => IdentIdName::ColumnIdent {
                column: "wallet".to_string(),
                val: wallet_id.to_raw(),
                rec: true,
            },
        };

        get_entity_list_view::<CurrencyTransactionView>(
            self.db,
            TABLE_NAME.to_string(),
            &ident,
            pagination,
        )
        .await
    }

    pub(crate) async fn create_init_record(
        &self,
        wallet_id: &Thing,
        currency: CurrencySymbol,
    ) -> CtxResult<CurrencyTransaction> {
        let mut res = self.db.query(format!("INSERT INTO {TABLE_NAME} {{ wallet: $wallet, with_wallet: $with_wallet, currency: $currency, balance: 0 }}"))
            .bind(( "wallet", wallet_id.clone()))
            .bind(( "with_wallet",  Thing::from((WALLET_TABLE, "init_wallet"))))
            .bind(( "currency",  currency))
            .await
            .map_err(CtxError::from(self.ctx))?;

        let data = res.take::<Option<CurrencyTransaction>>(0)?;
        Ok(data.unwrap())
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<CurrencyTransaction> {
        let opt =
            get_entity::<CurrencyTransaction>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

    pub(crate) fn build_transfer_qry<'b>(
        query: Query<'b, surrealdb::engine::any::Any>,
        wallet_from: &Thing,
        wallet_to: &Thing,
        amount: i64,
        currency: &CurrencySymbol,
        gateway_tx: Option<Thing>,
        description: Option<String>,
        tx_type: TransactionType,
        uniq: &str,
    ) -> Query<'b, surrealdb::engine::any::Any> {
        let mut qry = query
        .query(format!(
            "
            LET ${uniq}_lock_id = time::now() + 10s;
            UPDATE ${uniq}_w_from_id SET lock_id = ${uniq}_lock_id;
            LET ${uniq}_upd_lck = UPDATE ${uniq}_w_to_id SET lock_id = ${uniq}_lock_id;
            IF array::len(${uniq}_upd_lck) == 0 {{
                CREATE ${uniq}_w_to_id SET lock_id = ${uniq}_lock_id, transaction_head = {{}};
            }};
            LET ${uniq}_w_from = SELECT * FROM ONLY ${uniq}_w_from_id FETCH transaction_head[${uniq}_currency];
            LET ${uniq}_balance = ${uniq}_w_from.transaction_head[${uniq}_currency].balance OR 0;
            LET ${uniq}_tx_amt = type::number(${uniq}_amt);
            LET ${uniq}_updated_from_balance = ${uniq}_balance - ${uniq}_tx_amt;

            IF ${uniq}_w_from_id != ${uniq}_app_gateway_wallet_id && ${uniq}_updated_from_balance < 0 {{
                THROW \"{THROW_BALANCE_TOO_LOW}\";
            }};

            LET ${uniq}_tx_out = INSERT INTO {TABLE_NAME} {{
                id: rand::ulid(),
                wallet: ${uniq}_w_from_id,
                with_wallet: ${uniq}_w_to_id,
                currency: ${uniq}_currency,
                amount_out: ${uniq}_tx_amt,
                balance: ${uniq}_updated_from_balance,
                gateway_tx: ${uniq}_gateway_tx_id,
                lock_tx: ${uniq}_lock_tx_id,
                description: ${uniq}_description,
                type: ${uniq}_tx_type
            }} RETURN id;
            LET ${uniq}_tx_out_id = ${uniq}_tx_out[0].id;
            UPDATE ${uniq}_w_from_id SET transaction_head[${uniq}_currency] = ${uniq}_tx_out_id, lock_id = NONE;
    
            LET ${uniq}_w_to = SELECT * FROM ONLY ${uniq}_w_to_id FETCH transaction_head[${uniq}_currency];
            LET ${uniq}_balance_to = ${uniq}_w_to.transaction_head[${uniq}_currency].balance OR 0;

            LET ${uniq}_tx_in = INSERT INTO {TABLE_NAME} {{
                id: rand::ulid(),
                wallet: ${uniq}_w_to_id,
                with_wallet: ${uniq}_w_from_id,
                currency: ${uniq}_currency,
                amount_in: ${uniq}_tx_amt,
                balance: ${uniq}_balance_to + ${uniq}_tx_amt,
                gateway_tx: ${uniq}_gateway_tx_id,
                lock_tx: ${uniq}_lock_tx_id,
                description: ${uniq}_description,
                type: ${uniq}_tx_type
            }} RETURN id;
            LET ${uniq}_tx_in_id = ${uniq}_tx_in[0].id;
            UPDATE ${uniq}_w_to_id SET transaction_head[${uniq}_currency] = ${uniq}_tx_in_id, lock_id = NONE;
        "
        ));

        qry = qry
            .bind((format!("{uniq}_tx_type"), tx_type))
            .bind((format!("{uniq}_w_from_id"), wallet_from.clone()))
            .bind((format!("{uniq}_w_to_id"), wallet_to.clone()))
            .bind((format!("{uniq}_amt"), amount))
            .bind((format!("{uniq}_currency"), currency.clone()))
            .bind((
                format!("{uniq}_app_gateway_wallet_id"),
                APP_GATEWAY_WALLET.clone(),
            ))
            .bind((format!("{uniq}_gateway_tx_id"), gateway_tx))
            .bind((
                format!("{uniq}_description"),
                description.unwrap_or_default(),
            ));

        qry
    }
}
