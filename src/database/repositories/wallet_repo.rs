use async_trait::async_trait;
use surrealdb::sql::Thing;

use crate::database::repository_impl::Repository;
use crate::database::repository_traits::RepositoryCore;
use crate::entities::wallet::{
    get_user_lock_wallet_id, get_user_wallet_id,
    CurrencySymbol, WalletBalanceView, WalletBalancesView, WalletCurrencyTxHeads,
    WalletEntity, APP_GATEWAY_WALLET, TABLE_NAME, THROW_WALLET_LOCKED,
    TRANSACTION_HEAD_F,
};
use crate::entities::wallet::balance_transaction_entity::{
    BalanceTransactionDbService,
    TABLE_NAME as TRANSACTION_TABLE,
};
use crate::interfaces::repositories::wallet_ifce::WalletRepositoryInterface;
use crate::middleware::ctx::Ctx;
use crate::middleware::error::AppError;
use crate::middleware::utils::db_utils::{
    record_exists, IdentIdName,
};

#[async_trait]
impl WalletRepositoryInterface for Repository<WalletEntity> {
    async fn get_user_balances(&self, user_id: &Thing) -> Result<WalletBalancesView, surrealdb::Error> {
        let balance = self.get_user_balance(user_id).await?;
        let balance_locked = self.get_user_balance_locked(user_id).await?;
        Ok(WalletBalancesView {
            id: user_id.clone(),
            balance,
            balance_locked,
        })
    }

    async fn get_user_balance(&self, user_id: &Thing) -> Result<WalletBalanceView, surrealdb::Error> {
        let user_wallet_id = get_user_wallet_id(user_id);
        self.get_balance(&user_wallet_id).await
    }

    async fn get_user_balance_locked(&self, user_id: &Thing) -> Result<WalletBalanceView, surrealdb::Error> {
        let user_wallet_id = get_user_lock_wallet_id(user_id);
        self.get_balance(&user_wallet_id).await
    }

    async fn get_balance(&self, wallet_id: &Thing) -> Result<WalletBalanceView, surrealdb::Error> {
        Self::is_wallet_id(wallet_id)?;
        if record_exists(&self.client, wallet_id).await.is_ok() {
            let opt = self.item_view_by_ident::<WalletBalanceView>(&IdentIdName::Id(wallet_id.clone()))
                .await?;
            opt.ok_or(surrealdb::Error::Db(surrealdb::error::Db::IdNotFound { rid: wallet_id.to_string() }))
        } else {
            Ok(WalletBalanceView {
                id: wallet_id.clone(),
                balance_usd: 0,
                balance_reef: 0,
                balance_eth: 0,
            })
        }
    }

    async fn init_app_gateway_wallet(&self) -> Result<WalletBalanceView, surrealdb::Error> {
        let wallet_id = APP_GATEWAY_WALLET.clone();
        Self::is_wallet_id(&wallet_id)?;
        
        if record_exists(&self.client, &wallet_id).await.is_ok() {
            return Err(surrealdb::Error::Api(surrealdb::error::Api::Query("Wallet already exists".to_string())));
        }

        // Dummy Ctx for BalanceTransactionDbService
        let ctx = Ctx::new(Ok("system".to_string()), false);

        let init_tx_usd = BalanceTransactionDbService {
            db: &self.client,
            ctx: &ctx,
        }
        .create_init_record(&wallet_id, CurrencySymbol::USD)
        .await
        .map_err(|e| surrealdb::Error::Api(surrealdb::error::Api::Query(e.error.to_string())))?;

        let init_tx_reef = BalanceTransactionDbService {
            db: &self.client,
            ctx: &ctx,
        }
        .create_init_record(&wallet_id, CurrencySymbol::REEF)
        .await
        .map_err(|e| surrealdb::Error::Api(surrealdb::error::Api::Query(e.error.to_string())))?;

        let init_tx_eth = BalanceTransactionDbService {
            db: &self.client,
            ctx: &ctx,
        }
        .create_init_record(&wallet_id, CurrencySymbol::ETH)
        .await
        .map_err(|e| surrealdb::Error::Api(surrealdb::error::Api::Query(e.error.to_string())))?;

        let wallet_entity = WalletEntity {
            id: wallet_id.id.to_raw(),
            transaction_head: WalletCurrencyTxHeads {
                usd: init_tx_usd.id,
                eth: init_tx_eth.id,
                reef: init_tx_reef.id,
            },
            r_created: None,
            r_updated: None,
        };

        let created_wallet = self.item_create(wallet_entity)
            .await?;

        Ok(WalletBalanceView {
            id: Thing::from((TABLE_NAME, created_wallet.id.as_str())),
            balance_usd: init_tx_usd.balance,
            balance_reef: init_tx_reef.balance,
            balance_eth: init_tx_eth.balance,
        })
    }
}

impl Repository<WalletEntity> {
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let curr_usd = CurrencySymbol::USD;
        let curr_reef = CurrencySymbol::REEF;
        let curr_eth = CurrencySymbol::ETH;
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS {TRANSACTION_HEAD_F} ON TABLE {TABLE_NAME} TYPE object;
    DEFINE FIELD IF NOT EXISTS {TRANSACTION_HEAD_F}.{curr_usd} ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>>;
    DEFINE FIELD IF NOT EXISTS {TRANSACTION_HEAD_F}.{curr_reef} ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>>;
    DEFINE FIELD IF NOT EXISTS {TRANSACTION_HEAD_F}.{curr_eth} ON TABLE {TABLE_NAME} TYPE option<record<{TRANSACTION_TABLE}>>;
    DEFINE FIELD IF NOT EXISTS lock_id ON TABLE {TABLE_NAME} TYPE option<string> ASSERT {{
    IF $before==NONE || $value==NONE || $before<time::now() {{
        RETURN true 
    }} ELSE {{
        THROW \"{THROW_WALLET_LOCKED}\" 
    }} }};
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    // DEFINE INDEX IF NOT EXISTS r_created_idx ON TABLE {TABLE_NAME} COLUMNS r_created;
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();

    ");
        let mutation = self.client.query(sql).await?;
        mutation.check().expect("should mutate wallet");
        Ok(())
    }

    fn is_wallet_id(wallet_id: &Thing) -> Result<(), surrealdb::Error> {
        if wallet_id.tb != TABLE_NAME {
            return Err(surrealdb::Error::Db(surrealdb::error::Db::IdInvalid { value: wallet_id.to_string() }));
        }
        Ok(())
    }
}
