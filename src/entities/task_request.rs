use crate::entities::wallet::wallet_entity::CurrencySymbol;
use crate::utils::validate_utils::deserialize_thing_or_string;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use surrealdb::types::{RecordId, SurrealValue};

/// TaskRequest entity - represents a task with rewards
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct TaskRequestEntity {
    #[serde(deserialize_with = "deserialize_thing_or_string")]
    pub id: String,

    // Polymorphic reference - can point to discussion or post
    pub belongs_to: RecordId,
    pub created_by: RecordId,

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

    pub wallet_id: RecordId,

    pub status: TaskRequestStatus,
    pub due_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone, SurrealValue)]
pub enum TaskRequestStatus {
    Init,
    InProgress,
    Completed,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, SurrealValue)]
pub enum TaskRequestType {
    Public,
    Private,
}

#[derive(Display, Clone, Debug, Serialize, Deserialize, SurrealValue)]
#[serde(tag = "type")]
pub enum RewardType {
    OnDelivery,
}

#[derive(EnumString, Display, Clone, Debug, Serialize, Deserialize, SurrealValue)]
#[serde(tag = "type")]
pub enum DeliverableType {
    PublicPost,
}

// Additional view/create structs
#[derive(Debug, Serialize)]
pub struct TaskRequestCreate {
    pub task_id: RecordId,
    pub wallet_id: RecordId,
    pub from_user: RecordId,
    pub belongs_to: RecordId,
    pub request_txt: String,
    pub deliverable_type: DeliverableType,
    pub r#type: TaskRequestType,
    pub reward_type: RewardType,
    pub currency: CurrencySymbol,
    pub acceptance_period: u64,
    pub delivery_period: u64,
    pub increase_tasks_nr_for_belongs: bool,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub struct TaskDonorForReward {
    pub amount: i64,
    pub id: RecordId,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub struct TaskParticipantUserView {
    pub id: RecordId,
    pub username: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub struct TaskParticipantForReward {
    pub id: RecordId,
    pub user: TaskParticipantUserView,
    pub status: crate::entities::task_request_user::TaskParticipantStatus,
    pub reward_tx: Option<RecordId>,
}

#[derive(Debug, Deserialize, SurrealValue)]
pub struct TaskForReward {
    #[serde(deserialize_with = "deserialize_thing_or_string")]
    pub id: String,
    pub belongs_to: RecordId,
    pub request_txt: String,
    pub currency: CurrencySymbol,
    pub donors: Vec<TaskDonorForReward>,
    pub participants: Vec<TaskParticipantForReward>,
    pub wallet: crate::entities::wallet::wallet_entity::Wallet,
    pub balance: Option<i64>,
}
