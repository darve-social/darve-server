use crate::entity::currency_transaction_entitiy::CurrencyTransactionDbService;
use sb_middleware::db;
use sb_middleware::utils::db_utils::{
    get_entity, get_entity_view, record_exists, with_not_found_err, IdentIdName, ViewFieldSelector,
};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;
use sb_middleware::utils::string_utils::get_string_thing;
use crate::entity::wallet_entitiy::{Wallet, WalletDbService};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EndowmentTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub external_tx_id: String,
    pub external_account_id: Option<String>,
    pub internal_tx: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

enum EndowmentSource {
    GooglePay,
    ApplePay,
}

pub struct EndowmentTransactionDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
    pub is_development: bool,
}

pub const TABLE_NAME: &str = "endowment_transaction";
// const USER_TABLE: &str = sb_user_auth::entity::local_user_entity::TABLE_NAME;
const TRANSACTION_TABLE: &str = crate::entity::currency_transaction_entitiy::TABLE_NAME;

impl<'a> EndowmentTransactionDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD external_tx_id ON TABLE {TABLE_NAME} TYPE string VALUE $before OR $value;
    DEFINE FIELD external_account_id ON TABLE {TABLE_NAME} TYPE string VALUE $before OR $value;
    DEFINE FIELD internal_tx ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>> VALUE $before OR $value;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();

    ");
        let mutation = self.db.query(sql).await?;

        &mutation.check().expect("should mutate endowmentTransaction");

        Ok(())
    }

    // creates endowmentTransaction
    pub(crate) async fn accept_endowment_tx(&self, internal_user: Thing, external_tx_id: String) -> CtxResult<()> {
        let wallet_service = WalletDbService { db: self.db, ctx: self.ctx, is_development: self.is_development};

        // init user wallet
        let bal = wallet_service.get_balance(internal_user).await?;


        let tx_qry = CurrencyTransactionDbService::get_tx_qry(wallet_from, wallet_to, amount, currency, false)?;
        let qry = format!("
        BEGIN TRANSACTION;

        COMMIT TRANSACTION;

        ");
        /*let endowment_tx = self
            .db
            .create(TABLE_NAME)
            .content(EndowmentTransaction{
                id: None,
                external_tx_id,
                external_account_id: None,
                internal_tx: Thing {},
                r_created: None,
                r_updated: None,
            })
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<Wallet>| v.unwrap())?;*/
        Ok(())
    }


    pub(crate) async fn create_withdrawal_tx(&self) -> CtxResult<()> {
Ok(())

    }

    fn get_wallet_id(&self) -> String {
        WalletDbService::get_wallet_id(get_string_thing("local_user:endowment_account".to_string()))
    }

    // pub async fn get(&self, ident: IdentIdName) -> CtxResult<EndowmentTransaction> {
    //     let opt = get_entity::<EndowmentTransaction>(&self.db, TABLE_NAME.to_string(), &ident).await?;
    //     with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    // }
}

