use std::{sync::Arc, time::Duration};

use crate::{
    entities::{
        task_request_user::TaskRequestUserStatus,
        wallet::{
            balance_transaction_entity::BalanceTransactionDbService,
            wallet_entity::{CurrencySymbol, Wallet, WalletDbService},
        },
    },
    middleware::{ctx::Ctx, mw_ctx::CtxState},
};
use serde::Deserialize;
use surrealdb::sql::Thing;
use tokio::task::JoinHandle;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct TaskDonor {
    amount: i64,
    id: Thing,
}

#[derive(Debug, Deserialize)]
pub struct TaskUser {
    id: Thing,
    user_id: Thing,
    status: TaskRequestUserStatus,
    reward_tx: Option<Thing>,
}

#[derive(Debug, Deserialize)]
pub struct Task {
    pub currency: CurrencySymbol,
    pub participants: Vec<TaskDonor>,
    pub users: Vec<TaskUser>,
    pub wallet: Wallet,
    pub balance: i64,
}

pub async fn run(state: Arc<CtxState>, delay: Duration) -> JoinHandle<()> {
    let state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(delay).await;

            let query = "SELECT *, transaction_head[currency].balance as balance FROM (
                     SELECT
                        wallet_id.* AS wallet,
                        currency,
                        wallet_id.transaction_head AS transaction_head,
                        ->task_request_user.{ status, id, user_id: out } AS users,
                        ->task_request_participation.{ id: out, amount: transaction.amount_out } AS participants
                    FROM task_request
                    WHERE created_at + <duration>string::concat(delivery_period, 'h') <= time::now()
                ) WHERE transaction_head[currency].balance > 2;";

            let res = state.db.client.query(query).await;

            if res.is_err() {
                continue;
            }

            let tasks = res.unwrap().take::<Vec<Task>>(0).unwrap_or(vec![]);

            for task in tasks {
                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    let ctx = Ctx::new(Ok("".to_string()), Uuid::new_v4(), false);

                    let wallet_db_service = BalanceTransactionDbService {
                        db: &state.db.client,
                        ctx: &ctx,
                    };

                    let delivered_users = task
                        .users
                        .iter()
                        .filter(|user| user.status == TaskRequestUserStatus::Delivered)
                        .collect::<Vec<&TaskUser>>();
                    if delivered_users.is_empty() {
                        for user in task.participants {
                            let user_wallet = WalletDbService::get_user_wallet_id(&user.id);
                            let _ = wallet_db_service
                                .transfer_currency(
                                    task.wallet.id.as_ref().unwrap(),
                                    &user_wallet,
                                    user.amount,
                                    &task.currency,
                                )
                                .await;
                        }
                    } else {
                        let task_users = delivered_users
                            .into_iter()
                            .filter(|u| u.reward_tx.is_none())
                            .collect::<Vec<&TaskUser>>();

                        let amount: u64 = task.balance as u64 / task_users.len() as u64;

                        for task_user in task_users {
                            let user_wallet =
                                WalletDbService::get_user_wallet_id(&task_user.user_id);
                            let _ = wallet_db_service
                                .transfer_task_reward(
                                    task.wallet.id.as_ref().unwrap(),
                                    &user_wallet,
                                    amount as i64,
                                    &task.currency,
                                    &task_user.id,
                                )
                                .await;
                        }
                    }
                });
            }
        }
    })
}
