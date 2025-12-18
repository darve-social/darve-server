use surrealdb::sql::Thing;

use crate::entities::wallet::{WalletBalancesView, WalletBalanceView};
use crate::interfaces::repositories::wallet_ifce::WalletRepositoryInterface;
use crate::middleware::ctx::Ctx;
use crate::middleware::error::{AppError, CtxResult};

pub struct WalletService<'a, R>
where
    R: WalletRepositoryInterface + Send + Sync,
{
    repository: &'a R,
    ctx: &'a Ctx,
}

impl<'a, R> WalletService<'a, R>
where
    R: WalletRepositoryInterface + Send + Sync,
{
    pub fn new(repository: &'a R, ctx: &'a Ctx) -> Self {
        Self { repository, ctx }
    }

    pub async fn get_user_balances(&self, user_id: &Thing) -> CtxResult<WalletBalancesView> {
        self.repository
            .get_user_balances(user_id)
            .await
            .map_err(|e| self.ctx.to_ctx_error(AppError::SurrealDb { source: e.to_string() }))
    }

    pub async fn get_user_balance(&self, user_id: &Thing) -> CtxResult<WalletBalanceView> {
        self.repository
            .get_user_balance(user_id)
            .await
            .map_err(|e| self.ctx.to_ctx_error(AppError::SurrealDb { source: e.to_string() }))
    }

    pub async fn get_user_balance_locked(&self, user_id: &Thing) -> CtxResult<WalletBalanceView> {
        self.repository
            .get_user_balance_locked(user_id)
            .await
            .map_err(|e| self.ctx.to_ctx_error(AppError::SurrealDb { source: e.to_string() }))
    }

    pub async fn get_balance(&self, wallet_id: &Thing) -> CtxResult<WalletBalanceView> {
        self.repository
            .get_balance(wallet_id)
            .await
            .map_err(|e| self.ctx.to_ctx_error(AppError::SurrealDb { source: e.to_string() }))
    }

}