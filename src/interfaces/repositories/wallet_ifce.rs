use async_trait::async_trait;
use surrealdb::sql::Thing;
use crate::database::repository_traits::RepositoryCore;
use crate::entities::wallet::{WalletBalancesView, WalletBalanceView};

#[async_trait]
pub trait WalletRepositoryInterface: RepositoryCore {
    async fn get_user_balances(&self, user_id: &Thing) -> Result<WalletBalancesView, surrealdb::Error>;
    async fn get_user_balance(&self, user_id: &Thing) -> Result<WalletBalanceView, surrealdb::Error>;
    async fn get_user_balance_locked(&self, user_id: &Thing) -> Result<WalletBalanceView, surrealdb::Error>;
    async fn get_balance(&self, wallet_id: &Thing) -> Result<WalletBalanceView, surrealdb::Error>;
    async fn init_app_gateway_wallet(&self) -> Result<WalletBalanceView, surrealdb::Error>;
}