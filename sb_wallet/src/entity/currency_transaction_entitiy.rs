use std::collections::HashMap;
use crate::entity::wallet_entitiy::CurrencySymbol;
use sb_middleware::db;
use sb_middleware::utils::db_utils::{get_entity, with_not_found_err, IdentIdName, QryBindingsVal};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::{to_value, Thing, Value};
use sb_middleware::error::AppResult;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurrencyTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub wallet: Thing,
    pub with_wallet: Thing,
    pub tx_ident: String,
    pub endowment: Option<Thing>,
    pub currency: CurrencySymbol,
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
const ENDOWMENT_TABLE: &str = crate::entity::endowment_wallet::TABLE_NAME;
const TRANSACTION_HEAD_F: &str = crate::entity::wallet_entitiy::TRANSACTION_HEAD_F;

impl<'a> CurrencyTransactionDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let curr_usd = CurrencySymbol::USD.to_string();

        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD wallet ON TABLE {TABLE_NAME} TYPE record<{WALLET_TABLE}>;
    DEFINE FIELD with_wallet ON TABLE {TABLE_NAME} TYPE record<{WALLET_TABLE}>;
    DEFINE FIELD tx_ident ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD endowment ON TABLE {TABLE_NAME} TYPE option<record<{ENDOWMENT_TABLE}>> ASSERT {{
    IF $this.balance > 0 && (type::is::none($this.amount_in) || type::is::none($this.amount_out)) && !record::exists($value) {{
        THROW \"Tried to make endowment but endowment tx not found\"
    }} ELSE {{
        RETURN true
    }}
}};
    DEFINE FIELD currency ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0
        ASSERT $value INSIDE ['{curr_usd}'];
    DEFINE FIELD prev_transaction ON TABLE {TABLE_NAME} TYPE record<{TABLE_NAME}>;
    DEFINE FIELD amount_in ON TABLE {TABLE_NAME} TYPE option<number>;
    DEFINE FIELD amount_out ON TABLE {TABLE_NAME} TYPE option<number>;
    DEFINE FIELD balance ON TABLE {TABLE_NAME} TYPE number;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.db.query(sql).await?;

        &mutation.check().expect("should mutate currencyTransaction");

        Ok(())
    }

    pub async fn move_amount(
        &self,
        wallet_from: &Thing,
        wallet_to: &Thing,
        amount: i64,
        currency: &CurrencySymbol,
    ) -> CtxResult<()> {
        let tx_qry = Self::get_tx_qry(wallet_from, wallet_to, amount, currency, false)?;
        let res = tx_qry.into_query(self.db).await?;
        res.check()?;
        Ok(())
    }

    pub(crate) async fn create_init_record(
        &self,
        wallet_id: Thing,
        currency: CurrencySymbol,
        balance: Option<i64>,
        endowment: Option<Thing>,
    ) -> CtxResult<CurrencyTransaction> {
        let record = CurrencyTransaction {
            id: None,
            wallet: wallet_id.clone(),
            with_wallet: Thing::from((WALLET_TABLE, "init_wallet")),
            tx_ident: wallet_id.id.to_raw(),
            endowment,
            currency,
            prev_transaction: Thing::from((TABLE_NAME, "zero_tx")),
            amount_in: None,
            amount_out: None,
            balance: balance.unwrap_or(0),
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

    pub(crate) fn get_tx_qry(
        wallet_from: &Thing,
        wallet_to: &Thing,
        amount: i64,
        currency: &CurrencySymbol,
        exclude_sql_transaction: bool,
    ) -> AppResult<QryBindingsVal<Value>> {
        let (begin_tx, commit_tx) = if exclude_sql_transaction { ("", "") } else { ("BEGIN TRANSACTION;", "COMMIT TRANSACTION;") };
        let qry = format!(
            "
        {begin_tx}

            LET $w_from = SELECT * FROM ONLY $w_from_id FETCH {TRANSACTION_HEAD_F};
            LET $w_to = SELECT * FROM ONLY $w_to_id;

            LET $updated_from_balance = $w_from.{TRANSACTION_HEAD_F}.balance - type::number($amt);

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
                amount_out: type::number($amt),
                balance: $updated_from_balance,
            }} RETURN id;

            LET $tx_out_id = $tx_out[0].id;

            UPDATE $w_from.id SET {TRANSACTION_HEAD_F}=$tx_out_id;

            LET $tx_in = INSERT INTO {TABLE_NAME} {{
                wallet: $w_to_id,
                with_wallet:$w_from_id,
                tx_ident: $tx_ident,
                currency: $currency,
                prev_transaction: $w_to.{TRANSACTION_HEAD_F}.id,
                amount_in: type::number($amt),
                balance: $w_to.{TRANSACTION_HEAD_F}.balance + type::number($amt),
            }} RETURN id;

            LET $tx_in_id = $tx_in[0].id;

            UPDATE $w_to.id SET {TRANSACTION_HEAD_F}=$tx_in_id;

        {commit_tx}
        ");
        let mut bindings = HashMap::new();
        bindings.insert("w_from_id".to_string(), to_value(wallet_from.clone()).map_err(|e| AppError::SurrealDb {source: e.to_string()})?);
        bindings.insert("w_to_id".to_string(), to_value(wallet_to.clone()).map_err(|e| AppError::SurrealDb {source: e.to_string()})?);
        bindings.insert("amt".to_string(), to_value(amount).map_err(|e| AppError::SurrealDb {source: e.to_string()})?);
        bindings.insert("currency".to_string(), to_value(currency.clone()).map_err(|e| AppError::SurrealDb {source: e.to_string()})?);
        Ok(QryBindingsVal::new(qry, bindings))
    }
}

