use crate::{
    access::{base::role::Role, discussion::DiscussionAccess, post::PostAccess, task::TaskAccess},
    database::client::Db,
    entities::{
        self,
        access_user::AccessUser,
        community::{
            discussion_entity::DiscussionDbService,
            post_entity::{PostDbService, PostType},
        },
        tag::SystemTags,
        task::task_request_entity::{
            DeliverableType, RewardType, TaskForReward, TaskParticipantForReward, TaskRequest,
            TaskRequestCreate, TaskRequestDbService, TaskRequestStatus, TaskRequestType,
            TABLE_NAME as TASK_TABLE_NAME,
        },
        task_donor::TaskDonor,
        task_request_user::{TaskParticipant, TaskParticipantStatus},
        user_auth::local_user_entity::{LocalUser, LocalUserDbService},
        wallet::{
            balance_transaction_entity::{BalanceTransactionDbService, TransactionType},
            wallet_entity::{CurrencySymbol, WalletDbService, TABLE_NAME as WALLET_TABLE_NAME},
        },
    },
    interfaces::repositories::{
        access::AccessRepositoryInterface, delivery_result::DeliveryResultRepositoryInterface,
        tags::TagsRepositoryInterface, task_donors::TaskDonorsRepositoryInterface,
        task_participants::TaskParticipantsRepositoryInterface,
        user_notifications::UserNotificationsInterface,
    },
    middleware::{
        ctx::Ctx,
        error::{AppError, AppResult, CtxResult},
        utils::{
            db_utils::{IdentIdName, ViewFieldSelector},
            string_utils::get_str_thing,
        },
    },
    models::view::{
        access::{DiscussionAccessView, PostAccessView, TaskAccessView},
        post::PostView,
        task::TaskRequestView,
    },
    services::notification_service::{NotificationService, OnCreatedTaskView},
};
use chrono::{DateTime, TimeDelta, Utc};
use entities::wallet::wallet_entity::check_transaction_custom_error;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use validator::Validate;

#[derive(Deserialize, Serialize, Debug)]
pub struct TaskView {
    pub id: Thing,
    pub wallet_id: Thing,
    pub request_txt: String,
    pub reward_type: RewardType,
    pub currency: CurrencySymbol,
    pub r#type: TaskRequestType,
    pub donors: Vec<TaskDonor>,
    pub participants: Vec<TaskParticipant>,
    pub acceptance_period: u64,
    pub delivery_period: u64,
    pub created_at: DateTime<Utc>,
    pub related_to: Option<Thing>,
    pub status: TaskRequestStatus,
}

impl ViewFieldSelector for TaskView {
    fn get_select_query_fields() -> String {
        "id, 
        reward_type,
        type,
        request_txt,
        acceptance_period,
        delivery_period,
        currency,
        wallet_id,
        created_at,
        status,
        ->task_relate.out[0] as related_to,
        ->task_donor.*.{id, transaction, amount, user: out} as donors,
        ->task_participant.{id:record::id(id),task:record::id(in),user:record::id(out),status, timelines} as participants"
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
    #[serde(default)]
    pub participants: Vec<String>,
    #[validate(range(min = 100))]
    pub offer_amount: Option<u64>,
    #[validate(range(min = 1))]
    pub acceptance_period: Option<u64>,
    #[validate(range(min = 1))]
    pub delivery_period: Option<u64>,
}

pub struct TaskService<'a, T, N, P, A, TG, DR>
where
    T: TaskParticipantsRepositoryInterface,
    P: TaskDonorsRepositoryInterface,
    N: UserNotificationsInterface,
    A: AccessRepositoryInterface,
    TG: TagsRepositoryInterface,
    DR: DeliveryResultRepositoryInterface,
{
    tasks_repository: TaskRequestDbService<'a>,
    users_repository: LocalUserDbService<'a>,
    posts_repository: PostDbService<'a>,
    notification_service: NotificationService<'a, N>,
    transactions_repository: BalanceTransactionDbService<'a>,
    discussions_repository: DiscussionDbService<'a>,
    task_donors_repository: &'a P,
    task_participants_repository: &'a T,
    default_period_seconds: u64,
    access_repository: &'a A,
    tags_repository: &'a TG,
    delivery_result_repository: &'a DR,
    db: &'a Db,
}

impl<'a, T, N, P, A, TG, DR> TaskService<'a, T, N, P, A, TG, DR>
where
    T: TaskParticipantsRepositoryInterface,
    N: UserNotificationsInterface,
    P: TaskDonorsRepositoryInterface,
    A: AccessRepositoryInterface,
    TG: TagsRepositoryInterface,
    DR: DeliveryResultRepositoryInterface,
{
    pub fn new(
        db: &'a Db,
        ctx: &'a Ctx,
        task_donors_repository: &'a P,
        task_participants_repository: &'a T,
        access_repository: &'a A,
        tags_repository: &'a TG,
        notification_service: NotificationService<'a, N>,
        delivery_result_repository: &'a DR,
    ) -> Self {
        Self {
            tasks_repository: TaskRequestDbService { db: &db, ctx: &ctx },
            users_repository: LocalUserDbService { db: &db, ctx: &ctx },
            posts_repository: PostDbService { db: &db, ctx: &ctx },
            transactions_repository: BalanceTransactionDbService { db: &db, ctx: &ctx },
            task_donors_repository,
            task_participants_repository,
            discussions_repository: DiscussionDbService { db: &db, ctx },
            default_period_seconds: 48 * 60 * 60,
            access_repository,
            tags_repository,
            delivery_result_repository,
            notification_service: notification_service,
            db: db,
        }
    }

    pub async fn get(&self, user_thing_id: &str, task_id: &str) -> AppResult<TaskRequestView> {
        let user = self.users_repository.get_by_id(user_thing_id).await?;
        let task_thing = get_str_thing(&task_id)?;
        let task_view = self
            .tasks_repository
            .get_by_id::<TaskAccessView>(&task_thing)
            .await?;

        if !TaskAccess::new(&task_view).can_view(&user) {
            return Err(AppError::Forbidden);
        }

        let task = self
            .tasks_repository
            .get_by_id::<TaskRequestView>(&task_thing)
            .await?;

        Ok(task)
    }

    pub async fn create_for_post(
        &self,
        user_id: &str,
        post_id: &str,
        data: TaskRequestInput,
    ) -> CtxResult<TaskRequest> {
        data.validate()?;

        let user = self.users_repository.get_by_id(user_id).await?;

        let post = self
            .posts_repository
            .get_view_by_id::<PostAccessView>(post_id, None)
            .await?;

        let participants = self.get_participants(&data).await?;

        if participants.is_empty() {
            if !PostAccess::new(&post).can_create_public_task(&user) {
                return Err(AppError::Forbidden.into());
            }
        } else {
            for participant in participants.iter() {
                if participant.id == user.id {
                    return Err(AppError::Forbidden.into());
                }
                if !PostAccess::new(&post).can_view(participant) {
                    return Err(AppError::Forbidden.into());
                }
            }
            if !PostAccess::new(&post).can_create_private_task(&user) {
                return Err(AppError::Forbidden.into());
            }
        };

        let r#type = if participants.is_empty() {
            TaskRequestType::Public
        } else {
            TaskRequestType::Private
        };

        let is_donate_value = data.offer_amount.map_or(false, |v| v > 0);
        let post_id = post.id.clone();
        if is_donate_value {
            let task = TaskAccessView {
                id: user.id.as_ref().unwrap().clone(),
                r#type: r#type.clone(),
                post: Some(post.clone()),
                discussion: None,
                users: vec![AccessUser {
                    role: Role::Owner.to_string(),
                    user: user.id.as_ref().unwrap().clone(),
                    created_at: Utc::now(),
                }],
            };

            if !TaskAccess::new(&task).can_donate(&user) {
                return Err(AppError::Forbidden.into());
            }
        }

        let task = self
            .create(
                &user,
                participants.iter().map(|u| u).collect(),
                r#type,
                data,
                post_id,
                true,
            )
            .await?;

        let _ = self
            .notification_service
            .on_created_task(
                &user,
                &task,
                OnCreatedTaskView::Post(&post),
                participants.iter().map(|u| u).collect(),
                is_donate_value,
            )
            .await;

        Ok(task)
    }

    pub async fn create_for_disc(
        &self,
        user_id: &str,
        disc_id: &str,
        data: TaskRequestInput,
    ) -> CtxResult<TaskRequest> {
        data.validate()?;

        let user = self.users_repository.get_by_id(user_id).await?;

        let discussion = self
            .discussions_repository
            .get_view_by_id::<DiscussionAccessView>(disc_id)
            .await?;

        let participants = self.get_participants(&data).await?;

        if participants.is_empty() {
            if !DiscussionAccess::new(&discussion).can_create_public_task(&user) {
                return Err(AppError::Forbidden.into());
            }
        } else {
            for participant in participants.iter() {
                if participant.id == user.id {
                    return Err(AppError::Forbidden.into());
                }
                if !DiscussionAccess::new(&discussion).can_view(participant) {
                    return Err(AppError::Forbidden.into());
                }
            }
            if !DiscussionAccess::new(&discussion).can_create_private_task(&user) {
                return Err(AppError::Forbidden.into());
            }
        };
        let r#type = if participants.is_empty() {
            TaskRequestType::Public
        } else {
            TaskRequestType::Private
        };
        let disc_id = discussion.id.clone();

        let is_donate_value = data.offer_amount.map_or(false, |v| v > 0);
        if is_donate_value {
            let task = TaskAccessView {
                id: user.id.as_ref().unwrap().clone(),
                r#type: r#type.clone(),
                post: None,
                discussion: Some(discussion.clone()),
                users: vec![AccessUser {
                    role: Role::Owner.to_string(),
                    user: user.id.as_ref().unwrap().clone(),
                    created_at: Utc::now(),
                }],
            };

            if !TaskAccess::new(&task).can_donate(&user) {
                return Err(AppError::Forbidden.into());
            }
        }

        let task = self
            .create(
                &user,
                participants.iter().map(|u| u).collect(),
                r#type,
                data,
                disc_id,
                false,
            )
            .await?;

        let _ = self
            .notification_service
            .on_created_task(
                &user,
                &task,
                OnCreatedTaskView::Disc(&discussion),
                participants.iter().map(|u| u).collect(),
                is_donate_value,
            )
            .await;

        Ok(task)
    }

    pub async fn upsert_donor(
        &self,
        task_id: &str,
        donor_id: &str,
        data: TaskDonorData,
    ) -> AppResult<TaskDonor> {
        let task_thing = get_str_thing(&task_id)?;
        let task_view = self
            .tasks_repository
            .get_by_id::<TaskAccessView>(&task_thing)
            .await?;
        let donor_thing = get_str_thing(&donor_id)?;
        let donor = self
            .users_repository
            .get_by_id(&donor_thing.id.to_raw())
            .await?;

        if !TaskAccess::new(&task_view).can_donate(&donor) {
            return Err(AppError::Forbidden);
        }

        let task = self
            .tasks_repository
            .get_by_id::<TaskView>(&task_thing)
            .await?;

        if task.status != TaskRequestStatus::Init || data.amount <= 0 {
            return Err(AppError::Forbidden.into());
        }

        let participant = task.donors.iter().find(|p| &p.user == &donor_thing);

        let user_wallet = WalletDbService::get_user_wallet_id(&donor_thing);
        let mut query = self.db.query("BEGIN");
        match participant {
            Some(p) => {
                if let Some(ref tx) = p.transaction {
                    let tx = self
                        .transactions_repository
                        .get(IdentIdName::Id(tx.clone()))
                        .await?;

                    query = BalanceTransactionDbService::build_transfer_qry(
                        query,
                        &task.wallet_id,
                        &user_wallet,
                        tx.amount_out.unwrap(),
                        &tx.currency,
                        None,
                        Some("Update donate".to_string()),
                        TransactionType::Refund,
                        "",
                    );
                }

                query = BalanceTransactionDbService::build_transfer_qry(
                    query,
                    &user_wallet,
                    &task.wallet_id,
                    data.amount as i64,
                    &task.currency,
                    None,
                    Some("Update donate".to_string()),
                    TransactionType::Donate,
                    "donate",
                );

                query = self.task_donors_repository.build_update_query(
                    query,
                    &p.id.as_ref().unwrap().id.to_raw(),
                    "$donate_tx_out_id",
                    data.amount as u64,
                    &task.currency.to_string(),
                );
            }
            None => {
                query = BalanceTransactionDbService::build_transfer_qry(
                    query,
                    &user_wallet,
                    &task.wallet_id,
                    data.amount as i64,
                    &task.currency,
                    None,
                    Some("Update donate".to_string()),
                    TransactionType::Donate,
                    "donate",
                );

                query = self.task_donors_repository.build_create_query(
                    query,
                    &task_thing.id.to_raw(),
                    &donor_thing.id.to_raw(),
                    "$donate_tx_out_id",
                    data.amount as u64,
                    &task.currency.to_string(),
                );
            }
        };

        let mut res = query.query("RETURN $task_donor;").query("COMMIT").await?;
        check_transaction_custom_error(&mut res)?;

        let response: Option<TaskDonor> = res.take(0)?;

        if participant.is_none() {
            let _ = self
                .access_repository
                .add(
                    [donor_thing.clone()].to_vec(),
                    [task.id.clone()].to_vec(),
                    Role::Donor.to_string(),
                )
                .await?;
        }

        self.notification_service
            .on_donate_task(&donor, &task_view)
            .await?;

        self.notification_service
            .on_update_balance(&donor_thing)
            .await?;

        Ok(response.unwrap())
    }

    pub async fn reject(&self, user_id: &str, task_id: &str) -> AppResult<TaskParticipant> {
        let user = self.users_repository.get_by_id(&user_id).await?;

        let task_thing = get_str_thing(&task_id)?;
        let task_access_view = self
            .tasks_repository
            .get_by_id::<TaskAccessView>(&task_thing)
            .await?;

        if !TaskAccess::new(&task_access_view).can_reject(&user) {
            return Err(AppError::Forbidden);
        }

        let task: TaskView = self
            .tasks_repository
            .get_by_id::<TaskView>(&task_thing)
            .await?;

        let task_user = task.participants.iter().find(|v| v.user == user_id);

        let result = self
            .task_participants_repository
            .update(
                &task_user.as_ref().unwrap().id,
                TaskParticipantStatus::Rejected.as_str(),
            )
            .await
            .map_err(|_| AppError::SurrealDb {
                source: format!("reject_task"),
            })?;

        let _ = self.try_to_process_reward(&task).await;

        self.access_repository
            .remove_by_user(
                user.id.as_ref().unwrap().clone(),
                [task.id.clone()].to_vec(),
            )
            .await?;

        let _ = self
            .notification_service
            .on_rejected_task(&user, &task_access_view)
            .await;

        Ok(result)
    }

    pub async fn accept(&self, user_id: &str, task_id: &str) -> AppResult<TaskParticipant> {
        let task_thing = get_str_thing(&task_id)?;
        let user = self.users_repository.get_by_id(&user_id).await?;

        let task_view = self
            .tasks_repository
            .get_by_id::<TaskAccessView>(&task_thing)
            .await?;

        if !TaskAccess::new(&task_view).can_accept(&user) {
            return Err(AppError::Forbidden);
        }

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

        if task
            .donors
            .iter()
            .any(|t| &t.user == user.id.as_ref().unwrap())
        {
            return Err(AppError::Forbidden.into());
        }

        let task_user = task.participants.iter().find(|v| v.user == user_id);

        let result = match task_user {
            Some(value) => {
                let result = self
                    .task_participants_repository
                    .update(&value.id, TaskParticipantStatus::Accepted.as_str())
                    .await
                    .map_err(|e| AppError::SurrealDb {
                        source: e.to_string(),
                    })?;

                let _ = self
                    .access_repository
                    .update(
                        user.id.as_ref().unwrap().clone(),
                        task.id.clone(),
                        Role::Participant.to_string(),
                    )
                    .await?;
                result
            }
            None => {
                let result = self
                    .task_participants_repository
                    .create(
                        &task.id.id.to_raw(),
                        &user_id,
                        TaskParticipantStatus::Accepted.as_str(),
                    )
                    .await
                    .map_err(|e| AppError::SurrealDb {
                        source: e.to_string(),
                    })?;
                let _ = self
                    .access_repository
                    .add(
                        [user.id.as_ref().unwrap().clone()].to_vec(),
                        [task.id.clone()].to_vec(),
                        Role::Participant.to_string(),
                    )
                    .await?;
                result
            }
        };

        if task.status != TaskRequestStatus::InProgress {
            self.tasks_repository
                .update_status(task.id, TaskRequestStatus::InProgress)
                .await?;
        }
        self.notification_service
            .on_accepted_task(&user, &task_view)
            .await?;

        Ok(result)
    }

    pub async fn deliver(
        &self,
        user_id: &str,
        task_id: &str,
        data: TaskDeliveryData,
    ) -> AppResult<TaskParticipant> {
        let task_thing = get_str_thing(&task_id)?;
        let user = self.users_repository.get_by_id(&user_id).await?;

        let task_view = self
            .tasks_repository
            .get_by_id::<TaskAccessView>(&task_thing)
            .await?;

        if !TaskAccess::new(&task_view).can_deliver(&user) {
            return Err(AppError::Forbidden);
        }

        let post = self
            .posts_repository
            .get_view_by_id::<PostView>(&data.post_id, Some(user_id))
            .await?;

        self.handle_delivery_post(&data.post_id, &task_view).await?;

        let task = self
            .tasks_repository
            .get_by_id::<TaskView>(&task_thing)
            .await?;

        let task_user = task
            .participants
            .iter()
            .find(|v| v.user == user_id && v.status == TaskParticipantStatus::Accepted);

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

        let mut transaction = self.db.query("BEGIN TRANSACTION;");
        transaction = self.task_participants_repository.build_update_query(
            transaction,
            &task_user.unwrap().id,
            TaskParticipantStatus::Delivered.as_str(),
        );
        transaction = self.delivery_result_repository.build_create_query(
            transaction,
            &task_user.unwrap().id,
            &post.id.id.to_raw(),
            None,
        );
        transaction = transaction
            .query("COMMIT TRANSACTION;")
            .query("RETURN $task_participant;");

        let mut res = transaction.await?;
        let delivery_result = res
            .take::<Option<TaskParticipant>>(res.num_statements() - 1)?
            .expect("Post delivery error");

        let _ = self.try_to_process_reward(&task).await;

        join_all(task.donors.into_iter().map(|d| {
            self.users_repository
                .add_credits(d.user, (d.amount / 100) as u16)
        }))
        .await;

        self.notification_service
            .on_deliver_task(&user, &task_view, &post)
            .await?;

        Ok(delivery_result)
    }

    pub(crate) async fn distribute_expired_tasks_rewards(&self) -> AppResult<()> {
        let tasks = self.tasks_repository.get_ready_for_payment().await?;
        join_all(tasks.into_iter().map(|t| self.process_reward(t))).await;
        Ok(())
    }

    async fn try_to_process_reward(&self, task: &TaskView) -> AppResult<()> {
        if task.r#type == TaskRequestType::Public {
            return Ok(());
        }

        let task = self
            .tasks_repository
            .get_ready_for_payment_by_id(task.id.clone())
            .await?;

        let all_participants_completed = task.participants.iter().all(|u| {
            [
                TaskParticipantStatus::Rejected,
                TaskParticipantStatus::Delivered,
            ]
            .contains(&u.status)
        });
        if !all_participants_completed {
            return Ok(());
        }

        self.process_reward(task).await?;
        Ok(())
    }

    async fn process_reward(&self, task: TaskForReward) -> AppResult<()> {
        if task.balance.is_none() {
            let _ = self
                .tasks_repository
                .update_status(task.id, TaskRequestStatus::Completed)
                .await;
            return Ok(());
        }

        let delivered_users = task
            .participants
            .iter()
            .filter(|user| user.status == TaskParticipantStatus::Delivered)
            .collect::<Vec<&TaskParticipantForReward>>();

        let wallet_id = task.wallet.id.as_ref().unwrap();

        let mut is_completed = true;
        if delivered_users.is_empty() {
            for p in task.donors {
                let user_wallet = WalletDbService::get_user_wallet_id(&p.id);
                let res = self
                    .transactions_repository
                    .transfer_currency(
                        wallet_id,
                        &user_wallet,
                        p.amount as i64,
                        &task.currency,
                        Some("Refund by task".to_owned()),
                        TransactionType::Refund,
                    )
                    .await;
                if res.is_ok() {
                    let _ = self.notification_service.on_update_balance(&p.id).await;
                } else {
                    is_completed = false;
                }
            }
        } else {
            let task_users: Vec<&TaskParticipantForReward> = delivered_users
                .into_iter()
                .filter(|u| u.reward_tx.is_none())
                .collect();

            let task_donors = task.donors.iter().map(|d| &d.id).collect::<Vec<&Thing>>();

            let amount: u64 = task.balance.unwrap() as u64 / task_users.len() as u64;
            for task_user in task_users {
                let user_wallet = WalletDbService::get_user_wallet_id(&task_user.user.id);
                let res = self
                    .transactions_repository
                    .transfer_task_reward(
                        wallet_id,
                        &user_wallet,
                        amount as i64,
                        &task.currency,
                        &task_user.id,
                        Some("Reward by task".to_owned()),
                    )
                    .await;
                if res.is_ok() {
                    let _ = self
                        .notification_service
                        .on_task_reward(&task_user.user, &task.id, &task.belongs_to, &task_donors)
                        .await;
                    let _ = self
                        .notification_service
                        .on_update_balance(&task_user.user.id)
                        .await;
                } else {
                    is_completed = false;
                }
            }
        }

        if is_completed {
            let _ = self
                .tasks_repository
                .update_status(task.id, TaskRequestStatus::Completed)
                .await;
        }

        Ok(())
    }

    async fn create(
        &self,
        user: &LocalUser,
        participants: Vec<&LocalUser>,
        r#type: TaskRequestType,
        data: TaskRequestInput,
        belongs_to: Thing,
        increase_tasks_nr_for_belongs: bool,
    ) -> CtxResult<TaskRequest> {
        let offer_currency = CurrencySymbol::USD;
        let user_thing = user.id.as_ref().unwrap();
        let mut query = self.db.query("BEGIN");

        let id = surrealdb::sql::Id::ulid();
        let task_data = TaskRequestCreate {
            belongs_to,
            r#type,
            from_user: user_thing.clone(),
            request_txt: data.content,
            deliverable_type: DeliverableType::PublicPost,
            reward_type: RewardType::OnDelivery,
            currency: offer_currency.clone(),
            acceptance_period: data
                .acceptance_period
                .unwrap_or(self.default_period_seconds),
            delivery_period: data.delivery_period.unwrap_or(self.default_period_seconds),
            increase_tasks_nr_for_belongs,
            task_id: Thing::from((TASK_TABLE_NAME, id.clone())),
            wallet_id: Thing::from((WALLET_TABLE_NAME, id)),
        };

        query = self.tasks_repository.build_create_query(query, &task_data);

        if let Some(amount) = data.offer_amount {
            let user_wallet = WalletDbService::get_user_wallet_id(&user.id.as_ref().unwrap());

            query = BalanceTransactionDbService::build_transfer_qry(
                query,
                &user_wallet,
                &task_data.wallet_id,
                amount as i64,
                &task_data.currency,
                None,
                Some("Donate by task".to_owned()),
                TransactionType::Donate,
                "donate",
            );

            query = self.task_donors_repository.build_create_query(
                query,
                &task_data.task_id.id.to_raw(),
                &user_thing.id.to_raw(),
                "$donate_tx_out_id",
                amount as u64,
                &task_data.currency.to_string(),
            );
        }

        let participant_ids = participants
            .into_iter()
            .map(|u| u.id.as_ref().unwrap().clone())
            .collect::<Vec<Thing>>();

        if !participant_ids.is_empty() {
            query = self.task_participants_repository.build_create_query(
                query,
                &task_data.task_id.id.to_raw(),
                participant_ids
                    .iter()
                    .map(|id| id.id.to_raw())
                    .collect::<Vec<String>>(),
                TaskParticipantStatus::Requested.as_str(),
            );
        };

        let mut res = query.query("RETURN $task;").query("COMMIT").await?;
        check_transaction_custom_error(&mut res)?;
        let task: Option<TaskRequest> = res.take(0)?;
        let task = task.unwrap();

        if !participant_ids.is_empty() {
            self.access_repository
                .add(
                    participant_ids,
                    [task.id.as_ref().unwrap().clone()].to_vec(),
                    Role::Candidate.to_string(),
                )
                .await?;
        }

        if let Some(_) = data.offer_amount {
            self.access_repository
                .add(
                    [user.id.as_ref().unwrap().clone()].to_vec(),
                    [task.id.as_ref().unwrap().clone()].to_vec(),
                    Role::Donor.to_string(),
                )
                .await?;
            let _ = self
                .notification_service
                .on_update_balance(&user_thing.clone())
                .await;
        } else {
            self.access_repository
                .add(
                    [user.id.as_ref().unwrap().clone()].to_vec(),
                    [task.id.as_ref().unwrap().clone()].to_vec(),
                    Role::Owner.to_string(),
                )
                .await?;
        };

        Ok(task)
    }

    async fn get_participants(&self, data: &TaskRequestInput) -> AppResult<Vec<LocalUser>> {
        let ids = data
            .participants
            .iter()
            .filter_map(|id| get_str_thing(id).ok())
            .collect::<Vec<Thing>>();

        if ids.is_empty() {
            return Ok(vec![]);
        }
        self.users_repository
            .get_by_ids(ids)
            .await
            .map_err(|e| e.into())
    }

    fn can_still_use(&self, start: DateTime<Utc>, period: Option<u64>) -> bool {
        match period {
            Some(value) => {
                start
                    .checked_add_signed(TimeDelta::seconds(value as i64))
                    .unwrap()
                    > Utc::now()
            }
            _ => true,
        }
    }

    fn check_delivery_post_access(
        &self,
        deliver_post_view: &PostAccessView,
        task_view: &TaskAccessView,
    ) -> AppResult<()> {
        let donor_role = Role::Donor.to_string();

        let donors = task_view
            .users
            .iter()
            .filter_map(|u| {
                if u.role == donor_role {
                    let mut user = LocalUser::default("".to_string());
                    user.id = Some(u.user.clone());
                    Some(user)
                } else {
                    None
                }
            })
            .collect::<Vec<LocalUser>>();

        let post_access = PostAccess::new(deliver_post_view);

        if !donors.iter().all(|d| post_access.can_view(d)) {
            return Err(AppError::Generic {
                description: "All donors must have view access to the delivery post".to_string(),
            }
            .into());
        }

        Ok(())
    }

    async fn update_role_for_delivery_post(&self, post_view: &PostAccessView) -> AppResult<()> {
        let post_owner_role = Role::Owner.to_string();
        let post_owner = post_view.users.iter().find(|u| u.role == post_owner_role);

        if post_owner.is_none() {
            return Ok(());
        }

        match post_view.r#type {
            PostType::Public => {
                self.access_repository
                    .remove_by_user(post_owner.unwrap().user.clone(), vec![post_view.id.clone()])
                    .await?
            }
            PostType::Private => {
                self.access_repository
                    .update(
                        post_owner.unwrap().user.clone(),
                        post_view.id.clone(),
                        Role::Member.to_string(),
                    )
                    .await?
            }
            _ => (),
        }

        Ok(())
    }

    async fn handle_delivery_post(
        &self,
        post_id: &str,
        task_view: &TaskAccessView,
    ) -> AppResult<()> {
        let post_view = self
            .posts_repository
            .get_view_by_id::<PostAccessView>(&post_id, None)
            .await?;

        if post_view.r#type == PostType::Idea {
            return Err(AppError::Forbidden);
        }

        self.check_delivery_post_access(&post_view, task_view)?;

        self.update_role_for_delivery_post(&post_view).await?;

        let _ = self
            .tags_repository
            .create_with_relate(
                [SystemTags::Delivery.as_str().to_string()].to_vec(),
                post_view.id.clone(),
            )
            .await;

        Ok(())
    }
}
