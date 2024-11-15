use sb_middleware::db;
use sb_middleware::utils::db_utils::{get_entity, get_entity_list, with_not_found_err, IdentIdName};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use sb_user_auth::entity::access_rule_entity::AccessRule;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use surrealdb::opt::PatchOp;
use surrealdb::sql::Thing;
use crate::entity::wallet_entitiy::{CurrencySymbol, Wallet};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurrencyTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub wallet: Thing,
    pub with_wallet: Thing,
    pub tx_ident: String,
    pub currency: String,
    pub prev_transaction: Thing,
    pub amount_in: Option<i64>,
    pub amount_out: Option<i64>,
    pub balance: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

pub struct CurrencyTransactionDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "currency_transaction";
const WALLET_TABLE: &str = crate::entity::wallet_entitiy::TABLE_NAME;
const TRANSACTION_HEAD_F: &str = crate::entity::wallet_entitiy::TRANSACTION_HEAD_F;

impl<'a> CurrencyTransactionDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let curr_usd = CurrencySymbol::USD.to_string();

        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD wallet ON TABLE {TABLE_NAME} TYPE record<{WALLET_TABLE}>;
    DEFINE FIELD with_wallet ON TABLE {TABLE_NAME} TYPE record<{WALLET_TABLE}>;
    DEFINE FIELD tx_ident ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD currency ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0
        ASSERT $value INSIDE ['{curr_usd}'];
    DEFINE FIELD prev_transaction ON TABLE {TABLE_NAME} TYPE record<{TABLE_NAME}>;
    DEFINE FIELD amount_in ON TABLE {TABLE_NAME} TYPE option<number>;
    DEFINE FIELD amount_out ON TABLE {TABLE_NAME} TYPE option<number>;
    DEFINE FIELD balance ON TABLE {TABLE_NAME} TYPE number;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.db
            .query(sql)
            .await?;

        &mutation.check().expect("should mutate currencyTransaction");

        Ok(())
    }

    pub async fn move_amount(&self, wallet_from: Thing,  wallet_to: Thing, amount: i64, currency: CurrencySymbol) -> CtxResult<()> {
        let qry = format!("
        BEGIN TRANSACTION;

            LET $w_from = SELECT * FROM $w_from_id FETCH {TRANSACTION_HEAD_F};
            LET $w_to = SELECT * FROM $w_to_id;

            LET $updated_from_balance = $w_from.{TRANSACTION_HEAD_F}.balance - $amt;

            IF $updated_from_balance < 0 {{
                THROW \"Not enough funds\";
            }};

            LET $tx_ident = rand::ulid();

            LET $tx_out = INSERT INTO {TABLE_NAME} {{
                wallet: $w_from_id,
                with_wallet:$w_to_id,
                tx_ident: $tx_ident,
                currency: $currency,
                prev_transaction: $w_from.{TRANSACTION_HEAD_F}.id,
                amount_out: $amt,
                balance: $updated_from_balance,
            }};

            UPDATE $w_from_id SET {TRANSACTION_HEAD_F}=$tx_out.id;

            LET $tx_in = INSERT INTO {TABLE_NAME} {{
                wallet: $w_to_id,
                with_wallet:$w_from_id,
                tx_ident: $tx_ident,
                currency: $currency,
                prev_transaction: $w_to.{TRANSACTION_HEAD_F}.id,
                amount_in: $amt,
                balance: $w_to.{TRANSACTION_HEAD_F}.balance + $amt,
            }};

            UPDATE $w_to_id SET {TRANSACTION_HEAD_F}=$tx_in.id;

        COMMIT TRANSACTION;
        ");

        let res = self.db.query(qry)
            .bind(("w_from_id", wallet_from))
            .bind(("w_to_id", wallet_to))
            .bind(("amt", amount))
            .bind(("currency", currency.to_string())).await?;
        res.check()?;
        Ok(())
    }

    /*async fn create_record(%self, record: CurrencyTransaction){

    self.db
            .create(TABLE_NAME)
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<CurrencyTransaction>| v.unwrap())
    }*/

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<CurrencyTransaction> {
        let opt = get_entity::<CurrencyTransaction>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }
}
