use chrono::{Datelike, Local, TimeZone, Weekday};
use rand::{rng, seq::SliceRandom};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use surrealdb::sql::Thing;
use tokio::sync::broadcast::Sender;
use tokio::sync::Mutex;

use crate::{
    database::client::Database,
    entities::{
        community::discussion_entity::TABLE_NAME as DISC_TABLE_NAME,
        task_request::TaskRequestType,
        task_request_user::TaskParticipantStatus,
        user_auth::local_user_entity::{
            LocalUserDbService, UserRole, TABLE_NAME as USER_TABLE_NAME,
        },
    },
    interfaces::repositories::task_request_ifce::TaskRequestRepositoryInterface,
    middleware::{
        ctx::Ctx,
        error::{AppError, AppResult},
        mw_ctx::AppEvent,
        utils::db_utils::{Pagination, QryOrder, ViewFieldSelector},
    },
    models::view::task::TaskRequestView,
    services::{
        notification_service::NotificationService,
        task_service::{TaskRequestInput, TaskService},
    },
};

#[derive(Debug, Deserialize, Clone)]
struct TaskData {
    description: String,
    amount: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TaskContentView {
    pub description: String,
}

impl ViewFieldSelector for TaskContentView {
    fn get_select_query_fields() -> String {
        "request_txt as description".to_string()
    }
}

pub struct DarveTasksUtils {
    super_data: Vec<TaskData>,
    weekly_data: Vec<TaskData>,
    db: Arc<Database>,
    darve_id: Arc<RwLock<Option<String>>>,
    ctx: Ctx,
    create_public_lock: Arc<Mutex<()>>,
}

impl DarveTasksUtils {
    pub fn new(db: Arc<Database>) -> Self {
        let super_tasks = include_str!("../../darve_super_tasks.json");
        let weekly = include_str!("../../darve_weekly_tasks.json");
        Self {
            db,
            ctx: Ctx::new(Ok("".to_string()), false),
            darve_id: Arc::new(RwLock::new(None)),
            weekly_data: serde_json::from_str(weekly).expect("Darve weekly tasks parse error"),
            super_data: serde_json::from_str(super_tasks).expect("Darve super tasks parse error"),
            create_public_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn create_public(
        &self,
        user_id: &str,
        sender_event: &Sender<AppEvent>,
    ) -> AppResult<Vec<TaskRequestView>> {
        let _lock = self.create_public_lock.lock().await;
        let darve_id = self.get_darve_profile_id().await?;

        let super_tasks = self.db.task_request
            .get_by_public_disc::<TaskRequestView>(
                &darve_id,
                &user_id,
                Some(TaskRequestType::Public),
                None,
                None,
                Some(false),
            )
            .await
            .unwrap_or(vec![]);

        if !super_tasks.is_empty() || self.super_data.is_empty() {
            return Ok(super_tasks);
        }

        let last_super_task = self.db.task_request
            .get_by_public_disc::<TaskContentView>(
                &darve_id,
                &user_id,
                Some(TaskRequestType::Public),
                Some(Pagination {
                    order_by: None,
                    order_dir: Some(QryOrder::DESC),
                    count: 1,
                    start: 0,
                }),
                Some(true),
                None,
            )
            .await
            .unwrap_or(vec![]);

        let index: i32 = match last_super_task.first() {
            Some(v) => self
                .super_data
                .iter()
                .position(|t| t.description == v.description)
                .map(|v| v as i32)
                .unwrap_or(-1),
            None => -1,
        };

        let next_index: usize = if (index + 1 as i32) >= self.super_data.len() as i32 {
            0
        } else {
            (index + 1) as usize
        };

        let next_task = &self.super_data[next_index];

        let task_service = TaskService::new(
            &self.db.client,
            &self.ctx,
            &self.db.task_request,
            &self.db.task_donors,
            &self.db.task_participants,
            &self.db.access,
            &self.db.tags,
            NotificationService::new(
                &self.db.client,
                &self.ctx,
                &sender_event,
                &self.db.user_notifications,
            ),
            &self.db.delivery_result,
        );

        let acceptance_period = self.seconds_until_end_of_week();
        let task = task_service
            .create_for_disc(
                &darve_id,
                &Thing::from((DISC_TABLE_NAME, darve_id.as_str())).to_raw(),
                TaskRequestInput {
                    content: next_task.description.clone(),
                    acceptance_period: Some(acceptance_period),
                    delivery_period: Some(7 * 24 * 60 * 60),
                    offer_amount: next_task.amount,
                    participants: vec![],
                },
            )
            .await?;

        let task_view = task_service
            .get(user_id, &format!("task_request:{}", task.id))
            .await?;

        Ok(vec![task_view])
    }

    pub async fn create_private(
        &self,
        user_id: &str,
        sender_event: &Sender<AppEvent>,
    ) -> AppResult<Vec<TaskRequestView>> {
        let darve_id = self.get_darve_profile_id().await?;

        let weekly_tasks = self.db.task_request
            .get_by_public_disc::<TaskRequestView>(
                &darve_id,
                &user_id,
                Some(TaskRequestType::Private),
                None,
                None,
                Some(false),
            )
            .await
            .unwrap_or(vec![]);

        if !weekly_tasks.is_empty() || self.weekly_data.is_empty() {
            return Ok(weekly_tasks);
        }

        let disc = Thing::from((DISC_TABLE_NAME, darve_id.as_str()));
        let tasks_content = self.db.task_request
            .get_by_user_and_disc::<TaskContentView>(
                &user_id,
                &disc.id.to_raw(),
                Some(TaskParticipantStatus::Delivered),
            )
            .await
            .unwrap_or(vec![])
            .into_iter()
            .map(|t| t.description)
            .collect::<Vec<String>>();

        let random_tasks = self.get_random_weekly_tasks(tasks_content);
        let mut tasks = Vec::with_capacity(3);

        let task_service = TaskService::new(
            &self.db.client,
            &self.ctx,
            &self.db.task_request,
            &self.db.task_donors,
            &self.db.task_participants,
            &self.db.access,
            &self.db.tags,
            NotificationService::new(
                &self.db.client,
                &self.ctx,
                &sender_event,
                &self.db.user_notifications,
            ),
            &self.db.delivery_result,
        );

        for task in random_tasks {
            let participants = vec![Thing::from((USER_TABLE_NAME, user_id)).to_raw()];

            let acceptance_period = self.seconds_until_end_of_week();
            let task = task_service
                .create_for_disc(
                    &darve_id,
                    &Thing::from((DISC_TABLE_NAME, darve_id.as_str())).to_raw(),
                    TaskRequestInput {
                        content: task.description,
                        participants: participants,
                        acceptance_period: Some(acceptance_period),
                        delivery_period: Some(7 * 24 * 60 * 60),
                        offer_amount: None,
                    },
                )
                .await?;
            tasks.push(
                task_service
                    .get(user_id, &format!("task_request:{}", task.id))
                    .await?,
            );
        }

        Ok(tasks)
    }

    async fn get_darve_profile_id(&self) -> AppResult<String> {
        {
            let id = self.darve_id.read().unwrap();
            if let Some(cached_id) = id.as_ref() {
                return Ok(cached_id.clone());
            }
        }

        let user_repository = LocalUserDbService {
            db: &self.db.client,
            ctx: &self.ctx,
        };

        let admins = user_repository
            .get_by_role(UserRole::Admin)
            .await
            .unwrap_or_default();

        let admin = admins.first().ok_or_else(|| AppError::Generic {
            description: "Admin not found".to_string(),
        })?;

        let admin_id = admin.id.as_ref().ok_or_else(|| AppError::Generic {
            description: "Admin ID missing".to_string(),
        })?;

        let id = admin_id.id.to_raw();

        {
            let mut data = self.darve_id.write().unwrap();
            *data = Some(id.clone());
        }

        Ok(id)
    }

    fn get_random_weekly_tasks(&self, reject_content: Vec<String>) -> Vec<TaskData> {
        let mut unset_tasks = self
            .weekly_data
            .iter()
            .filter(|t| !reject_content.contains(&t.description))
            .collect::<Vec<&TaskData>>();

        if unset_tasks.is_empty() {
            return vec![];
        }

        let mut rng = rng();
        unset_tasks.shuffle(&mut rng);

        unset_tasks.into_iter().take(3).cloned().collect()
    }

    fn seconds_until_end_of_week(&self) -> u64 {
        let now = Local::now();
        let weekday = now.weekday();
        let days_until_sunday = (Weekday::Sun.number_from_monday() as i64
            - weekday.number_from_monday() as i64)
            .rem_euclid(7);

        let end_of_week = Local
            .with_ymd_and_hms(
                now.year(),
                now.month(),
                now.day() + days_until_sunday as u32,
                23,
                59,
                59,
            )
            .unwrap();

        (end_of_week - now).num_seconds().max(0) as u64
    }
}
