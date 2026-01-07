use std::sync::Arc;

use crate::{
    access::{base::role::Role, discussion::DiscussionAccess, post::PostAccess, task::TaskAccess},
    database::{client::Db, repositories::task_request_repo::TASK_REQUEST_TABLE_NAME},
    entities::{
        access_user::AccessUser,
        community::{
            discussion_entity::{DiscussionDbService, DiscussionType},
            post_entity::{CreatePost, PostDbService, PostType},
        },
        tag::SystemTags,
        task_donor::TaskDonor,
        task_request::{
            DeliverableType, RewardType, TaskForReward, TaskParticipantForReward,
            TaskRequestCreate, TaskRequestEntity, TaskRequestStatus, TaskRequestType,
        },
        task_request_user::{TaskParticipant, TaskParticipantResult, TaskParticipantStatus},
        user_auth::local_user_entity::{LocalUser, LocalUserDbService},
        wallet::{
            balance_transaction_entity::{BalanceTransactionDbService, TransactionType},
            wallet_entity::{
                check_transaction_custom_error, CurrencySymbol, WalletDbService,
                TABLE_NAME as WALLET_TABLE_NAME,
            },
        },
    },
    interfaces::{
        file_storage::FileStorageInterface,
        repositories::{
            access::AccessRepositoryInterface, tags::TagsRepositoryInterface,
            task_donors::TaskDonorsRepositoryInterface,
            task_participants::TaskParticipantsRepositoryInterface,
            task_request_ifce::TaskRequestRepositoryInterface,
            user_notifications::UserNotificationsInterface,
        },
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
        task::TaskRequestView,
    },
    services::notification_service::{NotificationService, OnCreatedTaskView},
    utils::file::convert::FileUpload,
};
use chrono::{DateTime, TimeDelta, Utc};
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

pub struct TaskService<'a, TR, T, N, P, A, TG>
where
    TR: TaskRequestRepositoryInterface,
    T: TaskParticipantsRepositoryInterface,
    P: TaskDonorsRepositoryInterface,
    N: UserNotificationsInterface,
    A: AccessRepositoryInterface,
    TG: TagsRepositoryInterface,
{
    tasks_repository: &'a TR,
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
    file_storage: Arc<dyn FileStorageInterface + Send + Sync>,
    db: &'a Db,
}

impl<'a, TR, T, N, P, A, TG> TaskService<'a, TR, T, N, P, A, TG>
where
    TR: TaskRequestRepositoryInterface,
    T: TaskParticipantsRepositoryInterface,
    N: UserNotificationsInterface,
    P: TaskDonorsRepositoryInterface,
    A: AccessRepositoryInterface,
    TG: TagsRepositoryInterface,
{
    pub fn new(
        db: &'a Db,
        ctx: &'a Ctx,
        tasks_repository: &'a TR,
        task_donors_repository: &'a P,
        task_participants_repository: &'a T,
        access_repository: &'a A,
        tags_repository: &'a TG,
        notification_service: NotificationService<'a, N>,
        file_storage: Arc<dyn FileStorageInterface + Send + Sync>,
    ) -> Self {
        Self {
            tasks_repository,
            users_repository: LocalUserDbService { db: &db, ctx: &ctx },
            posts_repository: PostDbService { db: &db, ctx: &ctx },
            transactions_repository: BalanceTransactionDbService { db: &db, ctx: &ctx },
            task_donors_repository,
            task_participants_repository,
            discussions_repository: DiscussionDbService { db: &db, ctx },
            default_period_seconds: 48 * 60 * 60,
            access_repository,
            tags_repository,
            file_storage,
            notification_service: notification_service,
            db: db,
        }
    }

    pub async fn get(&self, user_thing_id: &str, task_id: &str) -> AppResult<TaskRequestView> {
        let user = self.users_repository.get_by_id(user_thing_id).await?;
        let task_thing = get_str_thing(task_id)?;
        let task_view = self
            .tasks_repository
            .item_view_by_ident::<TaskAccessView>(&IdentIdName::Id(task_thing.clone()))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?
            .ok_or(AppError::EntityFailIdNotFound {
                ident: task_id.to_string(),
            })?;

        if !TaskAccess::new(&task_view).can_view(&user) {
            return Err(AppError::Forbidden);
        }

        let task = self
            .tasks_repository
            .item_view_by_ident::<TaskRequestView>(&IdentIdName::Id(task_thing))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?
            .ok_or(AppError::EntityFailIdNotFound {
                ident: task_id.to_string(),
            })?;

        Ok(task)
    }

    pub async fn create_for_post(
        &self,
        user_id: &str,
        post_id: &str,
        data: TaskRequestInput,
    ) -> CtxResult<TaskRequestEntity> {
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
    ) -> CtxResult<TaskRequestEntity> {
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
        let task_thing = get_str_thing(task_id)?;
        let task_view = self
            .tasks_repository
            .item_view_by_ident::<TaskAccessView>(&IdentIdName::Id(task_thing.clone()))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?
            .ok_or(AppError::EntityFailIdNotFound {
                ident: task_id.to_string(),
            })?;
        let donor_thing = Thing::from(("local_user", donor_id));
        let donor = self
            .users_repository
            .get_by_id(&donor_thing.id.to_raw())
            .await?;

        if !TaskAccess::new(&task_view).can_donate(&donor) {
            return Err(AppError::Forbidden);
        }

        let task = self
            .tasks_repository
            .item_view_by_ident::<TaskView>(&IdentIdName::Id(task_thing.clone()))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?
            .ok_or(AppError::EntityFailIdNotFound {
                ident: task_id.to_string(),
            })?;

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

        let task_thing = get_str_thing(task_id)?;
        let task_access_view = self
            .tasks_repository
            .item_view_by_ident::<TaskAccessView>(&IdentIdName::Id(task_thing.clone()))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?
            .ok_or(AppError::EntityFailIdNotFound {
                ident: task_id.to_string(),
            })?;

        if !TaskAccess::new(&task_access_view).can_reject(&user) {
            return Err(AppError::Forbidden);
        }

        let task: TaskView = self
            .tasks_repository
            .item_view_by_ident::<TaskView>(&IdentIdName::Id(task_thing))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?
            .ok_or(AppError::EntityFailIdNotFound {
                ident: task_id.to_string(),
            })?;

        let task_user = task.participants.iter().find(|v| v.user == user_id);

        let result = self
            .task_participants_repository
            .update(
                &task_user.as_ref().unwrap().id,
                TaskParticipantStatus::Rejected.as_str(),
                None,
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
        let task_thing = get_str_thing(task_id)?;
        let user = self.users_repository.get_by_id(&user_id).await?;

        let task_view = self
            .tasks_repository
            .item_view_by_ident::<TaskAccessView>(&IdentIdName::Id(task_thing.clone()))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?
            .ok_or(AppError::EntityFailIdNotFound {
                ident: task_id.to_string(),
            })?;

        if !TaskAccess::new(&task_view).can_accept(&user) {
            return Err(AppError::Forbidden);
        }

        let task = self
            .tasks_repository
            .item_view_by_ident::<TaskView>(&IdentIdName::Id(task_thing))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?
            .ok_or(AppError::EntityFailIdNotFound {
                ident: task_id.to_string(),
            })?;

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
                    .update(&value.id, TaskParticipantStatus::Accepted.as_str(), None)
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
                .await
                .map_err(|e| AppError::SurrealDb {
                    source: e.to_string(),
                })?;
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
        file: FileUpload,
    ) -> AppResult<TaskParticipant> {
        let task_thing = get_str_thing(task_id)?;
        let user = self.users_repository.get_by_id(&user_id).await?;

        let task_view = self
            .tasks_repository
            .item_view_by_ident::<TaskAccessView>(&IdentIdName::Id(task_thing.clone()))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?
            .ok_or(AppError::EntityFailIdNotFound {
                ident: task_id.to_string(),
            })?;

        if !TaskAccess::new(&task_view).can_deliver(&user) {
            return Err(AppError::Forbidden);
        }

        let task = self
            .tasks_repository
            .item_view_by_ident::<TaskView>(&IdentIdName::Id(task_thing))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?
            .ok_or(AppError::EntityFailIdNotFound {
                ident: task_id.to_string(),
            })?;

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

        let link = self
            .file_storage
            .upload(
                file.data,
                Some("tasks"),
                &format!("{}_{}_{}", user_id, task.id.id.to_raw(), file.file_name),
                file.content_type.as_deref(),
            )
            .await
            .map_err(|e| AppError::Generic { description: e })?;

        let task_participant_result = match task_view.discussion.as_ref() {
            Some(ref d) if d.r#type == DiscussionType::Private => TaskParticipantResult {
                link: Some(link),
                post: None,
            },
            _ => {
                let post = self
                    .posts_repository
                    .create(CreatePost {
                        belongs_to: DiscussionDbService::get_profile_discussion_id(
                            &user.id.as_ref().unwrap(),
                        ),
                        title: task.request_txt.to_string(),
                        content: Some(task.request_txt.clone()),
                        media_links: Some(vec![link]),
                        created_by: user.id.as_ref().unwrap().clone(),
                        id: PostDbService::get_new_post_thing(),
                        r#type: PostType::Public,
                    })
                    .await?;
                let _ = self
                    .tags_repository
                    .create_with_relate(
                        [SystemTags::Delivery.as_str().to_string()].to_vec(),
                        post.id.as_ref().unwrap().clone(),
                    )
                    .await;
                TaskParticipantResult {
                    post: Some(post.id.as_ref().unwrap().clone()),
                    link: None,
                }
            }
        };

        let delivery_result = self
            .task_participants_repository
            .update(
                &task_user.unwrap().id,
                TaskParticipantStatus::Delivered.as_str(),
                Some(&task_participant_result),
            )
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;

        let _ = self.try_to_process_reward(&task).await;

        join_all(task.donors.into_iter().map(|d| {
            self.users_repository
                .add_credits(d.user, (d.amount / 100) as u16)
        }))
        .await;

        self.notification_service
            .on_deliver_task(&user, &task_view, &task_participant_result)
            .await?;

        Ok(delivery_result)
    }

    pub(crate) async fn distribute_expired_tasks_rewards(&self) -> AppResult<()> {
        let tasks = self
            .tasks_repository
            .get_ready_for_payment()
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;
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
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;

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
    ) -> CtxResult<TaskRequestEntity> {
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
            task_id: Thing::from((TASK_REQUEST_TABLE_NAME, id.clone())),
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
        let task: Option<TaskRequestEntity> = res.take(0)?;
        let task = task.unwrap();

        if !participant_ids.is_empty() {
            self.access_repository
                .add(
                    participant_ids,
                    [task.id.clone()].to_vec(),
                    Role::Candidate.to_string(),
                )
                .await?;
        }

        if let Some(_) = data.offer_amount {
            self.access_repository
                .add(
                    [user.id.as_ref().unwrap().clone()].to_vec(),
                    [task.id.clone()].to_vec(),
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
                    [task.id.clone()].to_vec(),
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
}
