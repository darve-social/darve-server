use crate::entity::currency_transaction_entitiy::CurrencyTransactionDbService;
use sb_middleware::db;
use sb_middleware::utils::db_utils::{
    get_entity, with_not_found_err, IdentIdName,
};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
use crate::entity::wallet_entitiy::{CurrencySymbol, WalletDbService, APP_GATEWAY_WALLET};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FundingTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub amount: i64,
    pub currency: CurrencySymbol,
    pub external_tx_id: String,
    pub external_account_id: Option<String>,
    pub internal_tx: Thing,
    pub user: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

enum EndowmentSource {
    GooglePay,
    ApplePay,
}

pub struct FundingTransactionDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "funding_transaction";
const USER_TABLE: &str = sb_user_auth::entity::local_user_entity::TABLE_NAME;
const TRANSACTION_TABLE: &str = crate::entity::currency_transaction_entitiy::TABLE_NAME;

impl<'a> FundingTransactionDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {

        let curr_usd = CurrencySymbol::USD.to_string();

        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD external_tx_id ON TABLE {TABLE_NAME} TYPE string VALUE $before OR $value;
    DEFINE FIELD external_account_id ON TABLE {TABLE_NAME} TYPE string VALUE $before OR $value;
    DEFINE FIELD internal_tx ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>> VALUE $before OR $value;
    DEFINE FIELD user ON TABLE {TABLE_NAME} TYPE record<{USER_TABLE}>;
    DEFINE INDEX user_idx ON TABLE {TABLE_NAME} COLUMNS user;
    DEFINE FIELD amount ON TABLE {TABLE_NAME} TYPE number;
    DEFINE FIELD currency ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0
        ASSERT $value INSIDE ['{curr_usd}'];
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();

    ");
        let mutation = self.db.query(sql).await?;

        mutation.check().expect("should mutate fundingTransaction");

        Ok(())
    }

    // creates fundingTransaction
    pub(crate) async fn accept_endowment_tx(&self, user: &Thing, external_account: String, external_tx_id: String, amount: i64, currency_symbol: CurrencySymbol) -> CtxResult<Thing> {
        let wallet_service = WalletDbService { db: self.db, ctx: self.ctx};

        let user_wallet = WalletDbService::get_user_wallet_id(user);
        // init user wallet
        let _ = wallet_service.get_balance(&user_wallet).await?;
        
        let gwy_wallet = APP_GATEWAY_WALLET.clone();
        let _ = wallet_service.get_balance(&gwy_wallet).await?;
        let fund_tx_id = Thing::from((TABLE_NAME, Id::ulid()));


        let funding_2_user_tx = CurrencyTransactionDbService::get_tx_qry(&gwy_wallet, &user_wallet, amount, &currency_symbol, Some(fund_tx_id.clone()), true)?;
        let funding_2_user_qry = funding_2_user_tx.get_query_string();

        let fund_qry = format!("
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

           {funding_2_user_qry}

            RETURN $fund_tx[0].id;
        COMMIT TRANSACTION;

        ");
        let qry = self.db.query(fund_qry)
            .bind(("fund_tx_id", fund_tx_id))
            .bind(("fund_amt", amount))
            .bind(("user", user.clone()))
            .bind(("ext_tx", external_tx_id))
            .bind(("ext_account_id", external_account))
            .bind(("currency", currency_symbol));
        
        let qry = funding_2_user_tx.get_bindings().iter().fold(qry, |q, item|{
            q.bind((item.0.clone(), item.1.clone()))
        });

        let mut fund_res = qry.await?;

        fund_res=fund_res.check()?;
        let res:Option<Thing> = fund_res.take(0)?;
        res.ok_or(self.ctx.to_ctx_error(AppError::Generic {description:"Error in endowment tx".to_string()}))
    }


    pub(crate) async fn create_withdrawal_tx(&self) -> CtxResult<()> {
        todo!()
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<FundingTransaction> {
        let opt = get_entity::<FundingTransaction>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }
}

