use crate::{
    database::client::Db,
    entities::{
        community::{
            discussion_entity::{DiscussionDbService, TABLE_NAME as DISC_TB_NAME},
            post_entity::{PostDbService, TABLE_NAME as POST_TB_NAME},
        },
        task::task_request_entity::{
            DeliverableType, RewardType, TaskParticipantForReward, TaskRequest, TaskRequestCreate,
            TaskRequestDbService, TaskRequestType,
        },
        task_donor::TaskDonor,
        task_request_user::{TaskParticipant, TaskParticipantResult, TaskParticipantStatus},
        user_auth::local_user_entity::LocalUserDbService,
        wallet::{
            balance_transaction_entity::BalanceTransactionDbService,
            wallet_entity::{CurrencySymbol, WalletDbService},
        },
    },
    interfaces::repositories::{
        task_donors::TaskDonorsRepositoryInterface,
        task_participants::TaskParticipantsRepositoryInterface,
        task_relates::TaskRelatesRepositoryInterface,
        user_notifications::UserNotificationsInterface,
    },
    middleware::{
        ctx::Ctx,
        error::{AppError, AppResult, CtxResult},
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
    pub donors: Vec<TaskDonor>,
    pub participants: Vec<TaskParticipant>,
    pub acceptance_period: u16,
    pub delivery_period: u16,
    pub created_at: DateTime<Utc>,
    pub related_to: Option<Thing>,
}

impl ViewFieldSelector for TaskView {
    fn get_select_query_fields() -> String {
        "id, 
        reward_type,
        acceptance_period,
        delivery_period,
        currency,
        wallet_id,
        created_at,
        ->task_relate.out[0] as related_to,
        ->task_donor.*.{id, transaction, user: out} as donors,
        ->task_participant.{id:record::id(id),task:record::id(in),user:record::id(out),status, timelines} as participants,
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

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct TaskRequestInput {
    #[validate(length(min = 5, message = "Min 5 characters for content"))]
    pub content: String,
    pub participant: Option<String>,
    #[validate(range(min = 1))]
    pub offer_amount: Option<i64>,
    #[validate(range(min = 1))]
    pub acceptance_period: Option<u16>,
    #[validate(range(min = 1))]
    pub delivery_period: Option<u16>,
}

pub struct TaskService<'a, T, N, P, R>
where
    T: TaskParticipantsRepositoryInterface,
    P: TaskDonorsRepositoryInterface,
    N: UserNotificationsInterface,
    R: TaskRelatesRepositoryInterface,
{
    tasks_repository: TaskRequestDbService<'a>,
    users_repository: LocalUserDbService<'a>,
    posts_repository: PostDbService<'a>,
    notification_service: NotificationService<'a, N>,
    transactions_repository: BalanceTransactionDbService<'a>,
    discussions_repository: DiscussionDbService<'a>,
    task_donors_repository: &'a P,
    task_relates_repository: &'a R,
    task_participants_repository: &'a T,
    default_period_hours: u16,
}

impl<'a, T, N, P, R> TaskService<'a, T, N, P, R>
where
    T: TaskParticipantsRepositoryInterface,
    N: UserNotificationsInterface,
    P: TaskDonorsRepositoryInterface,
    R: TaskRelatesRepositoryInterface,
{
    pub fn new(
        db: &'a Db,
        ctx: &'a Ctx,
        event_sender: &'a Sender<AppEvent>,
        notification_repository: &'a N,
        task_donors_repository: &'a P,
        task_participants_repository: &'a T,
        task_relates_repository: &'a R,
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
            task_participants_repository,
            discussions_repository: DiscussionDbService { db: &db, ctx },
            default_period_hours: 48,
            task_relates_repository,
        }
    }

    pub async fn create(
        self,
        user_id: &str,
        data: TaskRequestInput,
        related_to: Option<Thing>,
    ) -> CtxResult<TaskRequest> {
        data.validate()?;

        let user_thing = get_str_thing(&user_id)?;
        let _ = self
            .users_repository
            .exists(IdentIdName::Id(user_thing.clone()))
            .await?;

        let _ = self.check_access(&user_thing, &related_to).await?;

        let (to_user, task_type) = match data.participant {
            Some(ref participant) if !participant.is_empty() => {
                let to_user_thing = get_str_thing(&participant)?;
                let user = self
                    .users_repository
                    .get(IdentIdName::Id(to_user_thing.clone()))
                    .await?;
                let _ = self.check_access(&to_user_thing, &related_to).await?;
                (Some(user), TaskRequestType::Close)
            }
            _ => (None, TaskRequestType::Open),
        };

        let offer_currency = CurrencySymbol::USD;

        let task = self
            .tasks_repository
            .create(TaskRequestCreate {
                from_user: user_thing.clone(),
                r#type: task_type,
                request_txt: data.content,
                deliverable_type: DeliverableType::PublicPost,
                reward_type: RewardType::OnDelivery,
                currency: offer_currency.clone(),
                acceptance_period: data.acceptance_period.unwrap_or(self.default_period_hours),
                delivery_period: data.delivery_period.unwrap_or(self.default_period_hours),
            })
            .await?;

        if let Some(id) = related_to {
            self.task_relates_repository
                .create(&task.id.as_ref().unwrap(), &id)
                .await?;
        }

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
                .task_participants_repository
                .create(
                    &task.id.as_ref().unwrap().id.to_raw(),
                    &user.id.as_ref().unwrap().id.to_raw(),
                    TaskParticipantStatus::Requested.as_str(),
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
            .await?;

        let donor_thing = get_str_thing(&donor_id)?;

        let _ = self
            .users_repository
            .exists(IdentIdName::Id(donor_thing.clone()))
            .await?;

        if data.amount <= 0 {
            return Err(AppError::Forbidden.into());
        }

        let is_some_accepted_or_delivered = task.participants.iter().any(|v| {
            v.status == TaskParticipantStatus::Accepted
                || v.status == TaskParticipantStatus::Delivered
        });

        if is_some_accepted_or_delivered {
            return Err(AppError::Forbidden.into());
        }

        let _ = self.check_access(&donor_thing, &task.related_to).await?;

        let participant = task.donors.iter().find(|p| p.user == donor_thing);
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
        let task_user = task.participants.iter().find(|v| v.user == user_id_id);

        let allow = task_user.map_or(false, |v| {
            v.status == TaskParticipantStatus::Requested
                || v.status == TaskParticipantStatus::Accepted
        });

        if !allow {
            return Err(AppError::Forbidden.into());
        }

        let _ = self.check_access(&user_thing, &task.related_to).await?;

        self.task_participants_repository
            .update(
                &task_user.as_ref().unwrap().id,
                TaskParticipantStatus::Rejected.as_str(),
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

        if !self.can_still_use(task.created_at, Some(task.acceptance_period)) {
            return Err(AppError::Generic {
                description: "The acceptance period has expired".to_string(),
            }
            .into());
        }

        let _ = self.check_access(&user_thing, &task.related_to).await?;

        if task.donors.iter().any(|t| t.user == user_thing) {
            return Err(AppError::Forbidden.into());
        }

        let user_id_id = user_thing.id.to_raw();
        let task_user = task.participants.iter().find(|v| v.user == user_id_id);

        match task.r#type {
            TaskRequestType::Open => {
                if task_user.is_some() {
                    return Err(AppError::Forbidden.into());
                };

                let _ = self
                    .task_participants_repository
                    .create(
                        &task.id.id.to_raw(),
                        &user_id_id,
                        TaskParticipantStatus::Accepted.as_str(),
                    )
                    .await
                    .map_err(|e| AppError::SurrealDb {
                        source: e.to_string(),
                    })?;
            }
            TaskRequestType::Close => {
                if task_user.map_or(true, |v| v.status != TaskParticipantStatus::Requested) {
                    return Err(AppError::Forbidden.into());
                }

                let _ = self
                    .task_participants_repository
                    .update(
                        &task_user.as_ref().unwrap().id,
                        TaskParticipantStatus::Accepted.as_str(),
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

        let user_id_id = user_thing.id.to_raw();
        let task_user = task
            .participants
            .iter()
            .find(|v| v.user == user_id_id && v.status == TaskParticipantStatus::Accepted);

        if task_user.is_none() {
            return Err(AppError::Forbidden.into());
        }

        let acceptance = task_user
            .unwrap()
            .timelines
            .last()
            .expect("get last timeline of the task");

        if !self.can_still_use(acceptance.date, Some(task.delivery_period)) {
            return Err(AppError::Generic {
                description: "The delivery period has expired".to_string(),
            }
            .into());
        }

        let post_thing = get_str_thing(&data.post_id)?;

        self.posts_repository
            .must_exist(IdentIdName::Id(post_thing))
            .await?;

        self.task_participants_repository
            .update(
                &task_user.unwrap().id,
                TaskParticipantStatus::Delivered.as_str(),
                Some(TaskParticipantResult {
                    urls: None,
                    post: Some(data.post_id),
                }),
            )
            .await
            .map_err(|_| AppError::SurrealDb {
                source: "deliver_task".to_string(),
            })?;

        let participant_ids = task
            .donors
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
                .participants
                .iter()
                .filter(|user| user.status == TaskParticipantStatus::Delivered)
                .collect::<Vec<&TaskParticipantForReward>>();

            let wallet_id = task.wallet.id.as_ref().unwrap();
            if delivered_users.is_empty() {
                for p in task.donors {
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
                let task_users: Vec<&TaskParticipantForReward> = delivered_users
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

    async fn check_access(&self, user_id: &Thing, related_to: &Option<Thing>) -> AppResult<()> {
        match related_to {
            Some(ref thing) => match thing.tb.as_str() {
                DISC_TB_NAME => self.check_disc_access(user_id, &thing).await,
                POST_TB_NAME => self.check_post_access(&thing).await,
                _ => Err(AppError::Forbidden),
            },
            None => Ok(()),
        }
    }

    async fn check_disc_access(&self, user_id: &Thing, disc_id: &Thing) -> AppResult<()> {
        let disc = self
            .discussions_repository
            .get_by_id(&disc_id.to_raw())
            .await
            .map_err(|_| AppError::Generic {
                description: "Forbidden".to_string(),
            })?;
        if !disc
            .private_discussion_user_ids
            .unwrap_or_default()
            .contains(&user_id)
        {
            return Err(AppError::Forbidden);
        }
        Ok(())
    }
    async fn check_post_access(&self, post_id: &Thing) -> AppResult<()> {
        self.posts_repository
            .get_by_id_with_access(&post_id.to_raw())
            .await
            .map_err(|_| AppError::Forbidden)?;
        Ok(())
    }
}
