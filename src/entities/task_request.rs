use crate::database::repository_traits::EntityWithId;
use crate::entities::wallet::wallet_entity::CurrencySymbol;
use crate::utils::validate_utils::{deserialize_thing_or_string_id};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use surrealdb::sql::Thing;

/// TaskRequest entity - represents a task with rewards
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskRequestEntity {
    #[serde(deserialize_with = "deserialize_thing_or_string_id")]
    pub id: String,

    // Polymorphic reference - can point to discussion or post
    pub belongs_to: Thing,
    pub created_by: Thing,

    pub request_txt: String,
    pub deliverable_type: DeliverableType,

    #[serde(rename = "type")]
    pub r#type: TaskRequestType,

    pub reward_type: RewardType,
    pub currency: CurrencySymbol,

    // ⚠️ CRITICAL: Keep field name "created_at" - same as old entity!
    pub created_at: DateTime<Utc>,

    pub acceptance_period: u64,
    pub delivery_period: u64,

    pub wallet_id: Thing,

    pub status: TaskRequestStatus,
    pub due_at: DateTime<Utc>,
}

impl EntityWithId for TaskRequestEntity {
    fn id_str(&self) -> Option<&str> {
        match self.id.is_empty() {
            true => None,
            false => Some(self.id.as_ref()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub enum TaskRequestStatus {
    Init,
    InProgress,
    Completed,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub enum TaskRequestType {
    Public,
    Private,
}

#[derive(Display, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RewardType {
    OnDelivery,
}

#[derive(EnumString, Display, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DeliverableType {
    PublicPost,
}

// Additional view/create structs
#[derive(Debug, Serialize)]
pub struct TaskRequestCreate {
    pub task_id: Thing,
    pub wallet_id: Thing,
    pub from_user: Thing,
    pub belongs_to: Thing,
    pub request_txt: String,
    pub deliverable_type: DeliverableType,
    pub r#type: TaskRequestType,
    pub reward_type: RewardType,
    pub currency: CurrencySymbol,
    pub acceptance_period: u64,
    pub delivery_period: u64,
    pub increase_tasks_nr_for_belongs: bool,
}

#[derive(Debug, Deserialize)]
pub struct TaskDonorForReward {
    pub amount: i64,
    pub id: Thing,
}

#[derive(Debug, Deserialize)]
pub struct TaskParticipantUserView {
    pub id: Thing,
    pub username: String,
}

#[derive(Debug, Deserialize)]
pub struct TaskParticipantForReward {
    pub id: Thing,
    pub user: TaskParticipantUserView,
    pub status: crate::entities::task_request_user::TaskParticipantStatus,
    pub reward_tx: Option<Thing>,
}

#[derive(Debug, Deserialize)]
pub struct TaskForReward {
    pub id: Thing,
    pub belongs_to: Thing,
    pub request_txt: String,
    pub currency: CurrencySymbol,
    pub donors: Vec<TaskDonorForReward>,
    pub participants: Vec<TaskParticipantForReward>,
    pub wallet: crate::entities::wallet::wallet_entity::Wallet,
    pub balance: Option<i64>,
}