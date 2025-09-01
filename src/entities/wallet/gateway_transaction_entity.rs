use std::fmt::Display;

use balance_transaction_entity::BalanceTransactionDbService;

use chrono::{DateTime, Utc};
use middleware::utils::db_utils::{get_entity, with_not_found_err, IdentIdName};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
use wallet_entity::{
    CurrencySymbol, WalletDbService, APP_GATEWAY_WALLET, TABLE_NAME as WALLET_TABLE_NAME,
};

use crate::database::client::Db;
use crate::database::surrdb_utils::get_entity_list;
use crate::entities::user_auth::local_user_entity;
use crate::entities::wallet::balance_transaction_entity::{self, TransactionType};
use crate::entities::wallet::wallet_entity::{self, check_transaction_custom_error};
use crate::middleware;
use crate::middleware::utils::db_utils::Pagination;

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub amount: i64,
    pub currency: CurrencySymbol,
    pub external_tx_id: String,
    pub user: Thing,
    pub withdraw_wallet: Option<Thing>,
    pub status: Option<String>,
    pub created_at: DateTime<Utc>,
    pub r#type: TransactionType,
    #[serde(default)]
    pub timelines: Vec<GatewayTransactionTimeline>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GatewayTransactionStatus {
    Pending,
    Completed,
    Failed,
    Init,
}

impl Display for GatewayTransactionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GatewayTransactionStatus::Pending => write!(f, "Pending"),
            GatewayTransactionStatus::Completed => write!(f, "Completed"),
            GatewayTransactionStatus::Failed => write!(f, "Failed"),
            GatewayTransactionStatus::Init => write!(f, "Init"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayTransactionTimeline {
    pub status: GatewayTransactionStatus,
    pub date: DateTime<Utc>,
}

pub struct GatewayTransactionDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "gateway_transaction";
const USER_TABLE: &str = local_user_entity::TABLE_NAME;

impl<'a> GatewayTransactionDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS external_tx_id ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD IF NOT EXISTS withdraw_wallet ON TABLE {TABLE_NAME} TYPE option<record<{WALLET_TABLE_NAME}>>;
    DEFINE FIELD IF NOT EXISTS status ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS user ON TABLE {TABLE_NAME} TYPE record<{USER_TABLE}>;
    DEFINE FIELD IF NOT EXISTS amount ON TABLE {TABLE_NAME} TYPE number;
    DEFINE FIELD IF NOT EXISTS currency ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD IF NOT EXISTS type ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS timelines ON TABLE {TABLE_NAME} TYPE array<{{ status: string, date: datetime }}>;
        
    DEFINE INDEX IF NOT EXISTS user_idx ON TABLE {TABLE_NAME} COLUMNS user;
    DEFINE INDEX IF NOT EXISTS type_idx ON TABLE {TABLE_NAME} COLUMNS type;
    DEFINE INDEX IF NOT EXISTS status_idx ON TABLE {TABLE_NAME} COLUMNS status;

    ");
        let mutation = self.db.query(sql).await?;

        mutation.check().expect("should mutate gatewayTransaction");

        Ok(())
    }

    pub async fn user_deposit_start(
        &self,
        id: Thing,
        user: Thing,
        amount: i64,
        currency: CurrencySymbol,
        external_tx_id: String,
    ) -> CtxResult<Thing> {
        let _ = self
            .db
            .query(format!(
                "INSERT INTO {TABLE_NAME} {{
                id: $id,
                amount: $amount,
                user: $user,
                currency: $currency,
                status: $status,
                type: $type,
                external_tx_id: $external_tx_id,
                timelines: [{{ status: $status, date: time::now() }}]
            }};"
            ))
            .bind(("id", id.clone()))
            .bind(("user", user))
            .bind(("external_tx_id", external_tx_id))
            .bind(("amount", amount))
            .bind(("currency", currency))
            .bind(("type", TransactionType::Deposit))
            .bind(("status", GatewayTransactionStatus::Init))
            .await?
            .check()?;

        Ok(id)
    }

    // creates gatewayTransaction
    pub(crate) async fn user_deposit_tx(
        &self,
        gateway_tx: Thing,
        external_tx_id: String,
        amount: i64,
        currency_symbol: CurrencySymbol,
        description: Option<String>,
    ) -> CtxResult<Thing> {
        let tx = self.get(IdentIdName::Id(gateway_tx)).await?;

        if tx.external_tx_id != external_tx_id {
            return Err(AppError::Generic {
                description: "Payment id is not valid".to_string(),
            }
            .into());
        }
        let user_wallet = WalletDbService::get_user_wallet_id(&tx.user);

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
            description,
            TransactionType::Deposit,
        )?;

        let gateway_2_user_qry = gateway_2_user_tx.get_query_string();

        let fund_qry = format!(
            "
        BEGIN TRANSACTION;
            UPDATE $gateway_tx SET
                status=$status,
                external_tx_id=$ext_tx,
                timelines+=[{{ status: $status, date: time::now() }}];
           {gateway_2_user_qry}
        COMMIT TRANSACTION;

        "
        );
        let qry = self
            .db
            .query(fund_qry)
            .bind(("gateway_tx", tx.id.as_ref().unwrap().clone()))
            .bind(("ext_tx", external_tx_id))
            .bind(("status", GatewayTransactionStatus::Completed));

        let qry = gateway_2_user_tx
            .get_bindings()
            .iter()
            .fold(qry, |q, item| q.bind((item.0.clone(), item.1.clone())));

        let mut fund_res = qry.await?;

        check_transaction_custom_error(&mut fund_res)?;
        Ok(tx.id.as_ref().unwrap().clone())
    }

    pub(crate) async fn user_withdraw_tx_start(
        &self,
        user: &Thing,
        amount: i64,
        description: Option<String>,
    ) -> CtxResult<Thing> {
        let user_wallet = WalletDbService::get_user_wallet_id(user);
        let wallet_to = WalletDbService::generate_id();
        let currency = CurrencySymbol::USD;

        let id = Self::generate_id();
        let query = BalanceTransactionDbService::get_transfer_qry(
            &user_wallet,
            &wallet_to,
            amount,
            &currency,
            Some(id.clone()),
            None,
            true,
            description,
            TransactionType::Withdraw,
        )?;

        let user_2_lock_qry = query.get_query_string();

        let qry = format!(
            "BEGIN TRANSACTION;
               {user_2_lock_qry}
            LET $fund_tx = INSERT INTO {TABLE_NAME} {{
                id: $id,
                amount: $fund_amt,
                user: $user,
                status: $status,
                withdraw_wallet: $withdraw_wallet,
                external_tx_id: $external_tx_id,
                currency: $currency,
                type: $type,
                timelines: [{{ status: $status, date: time::now() }}]
            }} RETURN id;
            LET $fund_tx_id = $fund_tx[0].id;
            $fund_tx_id;
        COMMIT TRANSACTION;"
        );

        let qry = self
            .db
            .query(qry)
            .bind(("id", id))
            .bind(("fund_amt", amount))
            .bind(("user", user.clone()))
            .bind(("external_tx_id", "".to_string()))
            .bind(("currency", currency))
            .bind(("wallet_withdraw", wallet_to.clone()))
            .bind(("type", TransactionType::Withdraw))
            .bind(("status", GatewayTransactionStatus::Pending));

        let qry = query
            .get_bindings()
            .into_iter()
            .fold(qry, |q, item| q.bind((item.0, item.1)));

        let mut fund_res = qry.await?;
        check_transaction_custom_error(&mut fund_res)?;

        let res: Option<Thing> = fund_res.take(fund_res.num_statements() - 1)?;
        res.ok_or(self.ctx.to_ctx_error(AppError::Generic {
            description: "Error in withdraw tx".to_string(),
        }))
    }

    pub(crate) async fn user_withdraw_tx_revert(
        &self,
        withdraw_tx_id: Thing,
        description: Option<String>,
    ) -> CtxResult<()> {
        let withdraw_tx = self.get(IdentIdName::Id(withdraw_tx_id.clone())).await?;
        let wallet_from = withdraw_tx
            .withdraw_wallet
            .ok_or(AppError::EntityFailIdNotFound {
                ident: "withdraw_wallet".to_string(),
            })?;
        let user_wallet = WalletDbService::get_user_wallet_id(&withdraw_tx.user);

        let query = BalanceTransactionDbService::get_transfer_qry(
            &wallet_from,
            &user_wallet,
            withdraw_tx.amount,
            &withdraw_tx.currency,
            Some(withdraw_tx_id.clone()),
            None,
            true,
            description,
            TransactionType::Withdraw,
        )?;

        let user_2_lock_qry = query.get_query_string();

        let qry = format!(
            "BEGIN TRANSACTION;
               {user_2_lock_qry}
               UPDATE $tx_id SET status=$status, timelines+=[{{ status: $status, date: time::now() }}];
             COMMIT TRANSACTION;"
        );

        let qry = self
            .db
            .query(qry)
            .bind(("tx_id", withdraw_tx_id))
            .bind(("status", GatewayTransactionStatus::Failed));

        let qry = query
            .get_bindings()
            .into_iter()
            .fold(qry, |q, item| q.bind((item.0, item.1)));

        let mut fund_res = qry.await?;
        check_transaction_custom_error(&mut fund_res)?;

        Ok(())
    }

    pub(crate) async fn user_withdraw_tx_complete(&self, withdraw_tx_id: Thing) -> CtxResult<()> {
        let withdraw_tx = self.get(IdentIdName::Id(withdraw_tx_id.clone())).await?;

        let wallet_from = withdraw_tx
            .withdraw_wallet
            .ok_or(AppError::EntityFailIdNotFound {
                ident: "withdraw_wallet".to_string(),
            })?;

        let query = BalanceTransactionDbService::get_transfer_qry(
            &wallet_from,
            &APP_GATEWAY_WALLET,
            withdraw_tx.amount,
            &withdraw_tx.currency,
            Some(withdraw_tx_id.clone()),
            None,
            true,
            None,
            TransactionType::Withdraw,
        )?;

        let user_2_lock_qry = query.get_query_string();

        let qry = format!(
            "BEGIN TRANSACTION;
               {user_2_lock_qry}
               UPDATE $tx_id SET status=$status, timelines+=[{{ status: $status, date: time::now() }}];
             COMMIT TRANSACTION;"
        );

        let qry = self
            .db
            .query(qry)
            .bind(("tx_id", withdraw_tx_id))
            .bind(("status", GatewayTransactionStatus::Completed));

        let qry = query
            .get_bindings()
            .into_iter()
            .fold(qry, |q, item| q.bind((item.0, item.1)));

        let mut fund_res = qry.await?;
        check_transaction_custom_error(&mut fund_res)?;
        Ok(())
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<GatewayTransaction> {
        let opt =
            get_entity::<GatewayTransaction>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

    pub async fn get_by_user(
        &self,
        user: &Thing,
        status: Option<GatewayTransactionStatus>,
        r#type: Option<TransactionType>,
        pagination: Option<Pagination>,
    ) -> CtxResult<Vec<GatewayTransaction>> {
        let ident = match (status, r#type) {
            (None, None) => IdentIdName::ColumnIdent {
                column: "user".to_string(),
                val: user.to_raw(),
                rec: true,
            },
            (None, Some(ref t)) => IdentIdName::ColumnIdentAnd(vec![
                IdentIdName::ColumnIdent {
                    column: "user".to_string(),
                    val: user.to_raw(),
                    rec: true,
                },
                IdentIdName::ColumnIdent {
                    column: "type".to_string(),
                    val: t.to_string(),
                    rec: false,
                },
            ]),
            (Some(ref s), None) => IdentIdName::ColumnIdentAnd(vec![
                IdentIdName::ColumnIdent {
                    column: "user".to_string(),
                    val: user.to_raw(),
                    rec: true,
                },
                IdentIdName::ColumnIdent {
                    column: "status".to_string(),
                    val: s.to_string(),
                    rec: false,
                },
            ]),
            (Some(ref s), Some(ref t)) => IdentIdName::ColumnIdentAnd(vec![
                IdentIdName::ColumnIdent {
                    column: "user".to_string(),
                    val: user.to_raw(),
                    rec: true,
                },
                IdentIdName::ColumnIdent {
                    column: "type".to_string(),
                    val: t.to_string(),
                    rec: false,
                },
                IdentIdName::ColumnIdent {
                    column: "status".to_string(),
                    val: s.to_string(),
                    rec: false,
                },
            ]),
        };

        let data = get_entity_list::<GatewayTransaction>(self.db, TABLE_NAME, &ident, pagination)
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;

        Ok(data)
    }

    pub fn generate_id() -> Thing {
        Thing::from((TABLE_NAME, Id::ulid()))
    }

    pub fn unknown_endowment_user_id(&self) -> Thing {
        Thing::try_from((USER_TABLE, "unrecognized_user_endowment_id")).expect("is valid")
    }
}
