use crate::{
    database::client::Db,
    entities::{
        community::post_entity::PostDbService,
        task::{
            task_request_entity::{
                DeliverableType, RewardType, TaskRequest, TaskRequestCreate, TaskRequestDbService,
                TaskRequestType, TaskUserForReward,
            },
            task_request_participation_entity::TaskRequestParticipation,
        },
        task_request_user::{TaskRequestUser, TaskRequestUserResult, TaskRequestUserStatus},
        user_auth::local_user_entity::LocalUserDbService,
        wallet::{
            balance_transaction_entity::BalanceTransactionDbService,
            wallet_entity::{CurrencySymbol, WalletDbService},
        },
    },
    interfaces::repositories::{
        task_participators::TaskParticipatorsRepositoryInterface,
        task_request_users::TaskRequestUsersRepositoryInterface,
        user_notifications::UserNotificationsInterface,
    },
    middleware::{
        ctx::Ctx,
        error::{AppError, AppResult},
        mw_ctx::AppEvent,
        utils::{
            db_utils::{IdentIdName, ViewFieldSelector},
            string_utils::get_str_thing,
        },
    },
    services::notification_service::NotificationService,
};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use tokio::sync::broadcast::Sender;
use validator::Validate;

#[derive(Deserialize, Serialize, Debug)]
pub struct TaskView {
    pub id: Thing,
    pub wallet_id: Thing,
    pub reward_type: RewardType,
    pub currency: CurrencySymbol,
    pub r#type: TaskRequestType,
    pub participants: Vec<TaskRequestParticipation>,
    pub to_users: Vec<TaskRequestUser>,
    pub acceptance_period: Option<u16>,
    pub delivery_period: u16,
    pub created_at: DateTime<Utc>,
}

impl ViewFieldSelector for TaskView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, 
        reward_type,
        acceptance_period,
        delivery_period,
        currency,
        wallet_id,
        created_at,
        ->task_request_participation.*.{id, amount, currency, transaction, user: out} as participants,
        ->task_request_user.{id:record::id(id),task:record::id(in),user:record::id(out),status} as to_users,
        type"
            .to_string()
    }
}
pub struct TaskDonorData {
    pub amount: u64,
}

pub struct TaskDeliveryData {
    pub post_id: String,
}

#[derive(Deserialize, Serialize, Validate)]
pub struct TaskRequestInput {
    #[validate(length(min = 5, message = "Min 5 characters for content"))]
    pub content: String,
    pub to_user: Option<String>,
    pub post_id: Option<String>,
    #[validate(range(min = 1))]
    pub offer_amount: Option<i64>,
    #[validate(range(min = 1))]
    pub acceptance_period: Option<u16>,
    #[validate(range(min = 1))]
    pub delivery_period: Option<u16>,
}

pub struct TaskService<'a, T, N, P>
where
    T: TaskRequestUsersRepositoryInterface,
    P: TaskParticipatorsRepositoryInterface,
    N: UserNotificationsInterface,
{
    tasks_repository: TaskRequestDbService<'a>,
    users_repository: LocalUserDbService<'a>,
    posts_repository: PostDbService<'a>,
    notification_service: NotificationService<'a, N>,
    transactions_repository: BalanceTransactionDbService<'a>,
    task_donors_repository: &'a P,
    task_users_repository: &'a T,
}

impl<'a, T, N, P> TaskService<'a, T, N, P>
where
    T: TaskRequestUsersRepositoryInterface,
    N: UserNotificationsInterface,
    P: TaskParticipatorsRepositoryInterface,
{
    pub fn new(
        db: &'a Db,
        ctx: &'a Ctx,
        event_sender: &'a Sender<AppEvent>,
        notification_repository: &'a N,
        task_donors_repository: &'a P,
        task_users_repository: &'a T,
    ) -> Self {
        Self {
            tasks_repository: TaskRequestDbService { db: &db, ctx: &ctx },
            users_repository: LocalUserDbService { db: &db, ctx: &ctx },
            posts_repository: PostDbService { db: &db, ctx: &ctx },
            transactions_repository: BalanceTransactionDbService { db: &db, ctx: &ctx },
            notification_service: NotificationService::new(
                db,
                ctx,
                event_sender,
                notification_repository,
            ),
            task_donors_repository,
            task_users_repository,
        }
    }

    pub async fn create(self, user_id: &str, data: TaskRequestInput) -> AppResult<TaskRequest> {
        let user_thing = get_str_thing(&user_id)?;
        let _ = self
            .users_repository
            .exists(IdentIdName::Id(user_thing.clone()))
            .await?;

        let (to_user, task_type) = match data.to_user {
            Some(ref to_user) if !to_user.is_empty() => {
                let to_user_thing = get_str_thing(&to_user)?;
                let user = self
                    .users_repository
                    .get(IdentIdName::Id(to_user_thing))
                    .await?;
                (Some(user), TaskRequestType::Close)
            }
            _ => (None, TaskRequestType::Open),
        };

        let offer_currency = CurrencySymbol::USD;

        let post = if let Some(ref post_id) = data.post_id {
            let post_thing = get_str_thing(&post_id.clone())?;

            self.posts_repository
                .must_exist(IdentIdName::Id(post_thing.clone()))
                .await?;

            Some(post_thing)
        } else {
            None
        };

        let task = self
            .tasks_repository
            .create(TaskRequestCreate {
                from_user: user_thing.clone(),
                on_post: post,
                r#type: task_type,
                request_txt: data.content,
                deliverable_type: DeliverableType::PublicPost,
                reward_type: RewardType::OnDelivery,
                currency: offer_currency.clone(),
                acceptance_period: data.acceptance_period,
                delivery_period: data.delivery_period.unwrap_or(10),
            })
            .await?;

        let offer_amount = data.offer_amount.unwrap_or(0);
        if offer_amount > 0 {
            let wallet_from = WalletDbService::get_user_wallet_id(&user_thing);

            let response = self
                .transactions_repository
                .transfer_currency(&wallet_from, &task.wallet_id, offer_amount, &offer_currency)
                .await?;

            let _ = self
                .task_donors_repository
                .create(
                    &task.id.as_ref().unwrap().id.to_raw(),
                    &user_thing.id.to_raw(),
                    &response.tx_out_id.id.to_raw(),
                    offer_amount as u64,
                    &offer_currency.to_string(),
                )
                .await
                .map_err(|e| AppError::SurrealDb {
                    source: e.to_string(),
                })?;
        }

        if let Some(ref user) = to_user {
            let _ = self
                .task_users_repository
                .create(
                    &task.id.as_ref().unwrap().id.to_raw(),
                    &user.id.as_ref().unwrap().id.to_raw(),
                    TaskRequestUserStatus::Requested.as_str(),
                )
                .await
                .map_err(|e| AppError::SurrealDb {
                    source: e.to_string(),
                })?;
        };

        let _ = self
            .notification_service
            .on_update_balance(&user_thing.clone(), &vec![user_thing.clone()])
            .await;

        if let Some(ref user) = to_user {
            let _ = self
                .notification_service
                .on_created_task(
                    &user_thing,
                    &task.id.as_ref().unwrap(),
                    &user.id.as_ref().unwrap(),
                )
                .await?;

            let _ = self
                .notification_service
                .on_received_task(
                    &user_thing,
                    &task.id.as_ref().unwrap(),
                    user.id.as_ref().unwrap(),
                )
                .await?;
        };
        Ok(task)
    }

    pub async fn upsert_donor(
        self,
        task_id: &str,
        donor_id: &str,
        data: TaskDonorData,
    ) -> AppResult<String> {
        let task_thing = get_str_thing(&task_id)?;
        let task = self
            .tasks_repository
            .get_by_id::<TaskView>(&task_thing)
            .await;
        let task = task?;
        let donor_thing = get_str_thing(&donor_id)?;

        let _ = self
            .users_repository
            .exists(IdentIdName::Id(donor_thing.clone()))
            .await?;
        if data.amount <= 0 {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        }

        let is_some_accepted_or_delivered = task.to_users.iter().any(|v| {
            v.status == TaskRequestUserStatus::Accepted
                || v.status == TaskRequestUserStatus::Delivered
        });

        if is_some_accepted_or_delivered {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        }

        let participant = task.participants.iter().find(|p| p.user == donor_thing);
        let user_wallet = WalletDbService::get_user_wallet_id(&donor_thing);
        let offer_id = match participant {
            Some(p) => {
                if let Some(ref tx) = p.transaction {
                    let tx = self
                        .transactions_repository
                        .get(IdentIdName::Id(tx.clone()))
                        .await?;

                    let _ = self
                        .transactions_repository
                        .transfer_currency(
                            &task.wallet_id,
                            &user_wallet,
                            tx.amount_out.unwrap(),
                            &tx.currency,
                        )
                        .await?;
                }

                let response = self
                    .transactions_repository
                    .transfer_currency(
                        &user_wallet,
                        &task.wallet_id,
                        data.amount as i64,
                        &task.currency,
                    )
                    .await?;

                let _ = self
                    .task_donors_repository
                    .update(
                        &p.id.as_ref().unwrap().id.to_raw(),
                        &response.tx_out_id.id.to_raw(),
                        data.amount as u64,
                        &task.currency.to_string(),
                    )
                    .await
                    .map_err(|e| AppError::SurrealDb {
                        source: e.to_string(),
                    })?;
                p.id.as_ref().unwrap().to_raw()
            }
            None => {
                let response = self
                    .transactions_repository
                    .transfer_currency(
                        &user_wallet,
                        &task.wallet_id,
                        data.amount as i64,
                        &task.currency,
                    )
                    .await?;

                let id = self
                    .task_donors_repository
                    .create(
                        &task_thing.id.to_raw(),
                        &donor_thing.id.to_raw(),
                        &response.tx_out_id.id.to_raw(),
                        data.amount as u64,
                        &task.currency.to_string(),
                    )
                    .await
                    .map_err(|e| AppError::SurrealDb {
                        source: e.to_string(),
                    })?;

                id
            }
        };

        self.notification_service
            .on_update_balance(&donor_thing, &vec![donor_thing.clone()])
            .await?;

        Ok(offer_id)
    }

    pub async fn reject(&self, user_id: &str, task_id: &str) -> AppResult<()> {
        let user_thing = get_str_thing(&user_id)?;
        let _ = self
            .users_repository
            .exists(IdentIdName::Id(user_thing.clone()))
            .await?;

        let task_thing = get_str_thing(&task_id)?;
        let task = self
            .tasks_repository
            .get_by_id::<TaskView>(&task_thing)
            .await?;
        let user_id_id = user_thing.id.to_raw();
        let task_user = task.to_users.iter().find(|v| v.user == user_id_id);

        let allow = task_user.map_or(false, |v| {
            v.status == TaskRequestUserStatus::Requested
                || v.status == TaskRequestUserStatus::Accepted
        });

        if !allow {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        }

        self.task_users_repository
            .update(
                &task_user.as_ref().unwrap().id,
                TaskRequestUserStatus::Rejected.as_str(),
                None,
            )
            .await
            .map_err(|_| AppError::SurrealDb {
                source: format!("reject_task"),
            })?;
        Ok(())
    }

    pub async fn accept(&self, user_id: &str, task_id: &str) -> AppResult<()> {
        let user_thing = get_str_thing(&user_id)?;
        let task_thing = get_str_thing(&task_id)?;
        let _ = self
            .users_repository
            .exists(IdentIdName::Id(user_thing.clone()))
            .await?;

        let task = self
            .tasks_repository
            .get_by_id::<TaskView>(&task_thing)
            .await?;

        if !self.can_still_use(task.created_at, task.acceptance_period) {
            return Err(AppError::Generic {
                description: "The acceptance period has expired".to_string(),
            }
            .into());
        }

        if task.participants.iter().any(|t| t.user == user_thing) {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        }

        let user_id_id = user_thing.id.to_raw();
        let task_user = task.to_users.iter().find(|v| v.user == user_id_id);

        match task.r#type {
            TaskRequestType::Open => {
                if task_user.is_some() {
                    return Err(AppError::Generic {
                        description: "Forbidden".to_string(),
                    }
                    .into());
                };

                let _ = self
                    .task_users_repository
                    .create(
                        &task.id.id.to_raw(),
                        &user_id_id,
                        TaskRequestUserStatus::Accepted.as_str(),
                    )
                    .await
                    .map_err(|e| AppError::SurrealDb {
                        source: e.to_string(),
                    })?;
            }
            TaskRequestType::Close => {
                if task_user.map_or(true, |v| v.status != TaskRequestUserStatus::Requested) {
                    return Err(AppError::Generic {
                        description: "Forbidden".to_string(),
                    }
                    .into());
                }

                let _ = self
                    .task_users_repository
                    .update(
                        &task_user.as_ref().unwrap().id,
                        TaskRequestUserStatus::Accepted.as_str(),
                        None,
                    )
                    .await
                    .map_err(|e| AppError::SurrealDb {
                        source: e.to_string(),
                    })?;
            }
        }
        Ok(())
    }

    pub async fn deliver(
        &self,
        user_id: &str,
        task_id: &str,
        data: TaskDeliveryData,
    ) -> AppResult<()> {
        let user_thing = get_str_thing(&user_id)?;
        let task_thing = get_str_thing(&task_id)?;
        let _ = self
            .users_repository
            .exists(IdentIdName::Id(user_thing.clone()))
            .await?;

        let task = self
            .tasks_repository
            .get_by_id::<TaskView>(&task_thing)
            .await?;

        if !self.can_still_use(task.created_at, Some(task.delivery_period)) {
            return Err(AppError::Generic {
                description: "The delivery period has expired".to_string(),
            }
            .into());
        }

        let user_id_id = user_thing.id.to_raw();
        let task_user = task
            .to_users
            .iter()
            .find(|v| v.user == user_id_id && v.status == TaskRequestUserStatus::Accepted);

        if task_user.is_none() {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        }

        let post_thing = get_str_thing(&data.post_id)?;

        self.posts_repository
            .must_exist(IdentIdName::Id(post_thing))
            .await?;

        self.task_users_repository
            .update(
                &task_user.unwrap().id,
                TaskRequestUserStatus::Delivered.as_str(),
                Some(TaskRequestUserResult {
                    urls: None,
                    post: Some(data.post_id),
                }),
            )
            .await
            .map_err(|_| AppError::SurrealDb {
                source: "deliver_task".to_string(),
            })?;

        let participant_ids = task
            .participants
            .iter()
            .map(|t| t.user.clone())
            .collect::<Vec<Thing>>();

        self.notification_service
            .on_deliver_task(&user_thing, task_thing.clone(), &participant_ids)
            .await?;

        Ok(())
    }

    pub(crate) async fn distribute_expired_tasks_rewards(&self) -> AppResult<()> {
        let tasks = self.tasks_repository.get_ready_for_payment().await?;

        for task in tasks {
            let delivered_users = task
                .users
                .iter()
                .filter(|user| user.status == TaskRequestUserStatus::Delivered)
                .collect::<Vec<&TaskUserForReward>>();

            let wallet_id = task.wallet.id.as_ref().unwrap();
            if delivered_users.is_empty() {
                for p in task.participants {
                    let user_wallet = WalletDbService::get_user_wallet_id(&p.id);
                    let res = self
                        .transactions_repository
                        .transfer_currency(wallet_id, &user_wallet, p.amount as i64, &task.currency)
                        .await;
                    if res.is_ok() {
                        let _ = self
                            .notification_service
                            .on_update_balance(&p.id, &vec![p.id.clone()])
                            .await;
                    }
                }
            } else {
                let task_users: Vec<&TaskUserForReward> = delivered_users
                    .into_iter()
                    .filter(|u| u.reward_tx.is_none())
                    .collect();

                let amount: u64 = task.balance as u64 / task_users.len() as u64;
                for task_user in task_users {
                    let user_wallet = WalletDbService::get_user_wallet_id(&task_user.user_id);
                    let res = self
                        .transactions_repository
                        .transfer_task_reward(
                            wallet_id,
                            &user_wallet,
                            amount as i64,
                            &task.currency,
                            &task_user.id,
                        )
                        .await;
                    if res.is_ok() {
                        let _ = self
                            .notification_service
                            .on_update_balance(&task_user.user_id, &vec![task_user.user_id.clone()])
                            .await;
                    }
                }
            }
        }

        Ok(())
    }

    fn can_still_use(&self, start: DateTime<Utc>, period: Option<u16>) -> bool {
        match period {
            Some(value) => {
                start
                    .checked_add_signed(TimeDelta::hours(value.into()))
                    .unwrap()
                    > Utc::now()
            }
            _ => true,
        }
    }
}
