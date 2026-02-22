use crate::database::surrdb_utils::record_id_to_raw;
use crate::entities::wallet::wallet_entity::CurrencySymbol;
use crate::utils::validate_utils::deserialize_thing_or_string;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;
use strum::{Display, EnumString};
use surrealdb::types::{ConversionError, Kind, RecordId, SurrealValue, Value};

/// A string wrapper for task IDs that handles both RecordId and String in SurrealValue
/// deserialization (SurrealValue derive ignores #[serde(deserialize_with)] attributes).
#[derive(Debug, Clone, PartialEq)]
pub struct TaskIdStr(pub String);

impl SurrealValue for TaskIdStr {
    fn kind_of() -> Kind {
        Kind::Any
    }
    fn is_value(value: &Value) -> bool {
        matches!(value, Value::RecordId(_) | Value::String(_))
    }
    fn into_value(self) -> Value {
        Value::String(self.0)
    }
    fn from_value(value: Value) -> Result<Self, surrealdb::Error> {
        match value {
            Value::RecordId(rid) => Ok(TaskIdStr(record_id_to_raw(&rid))),
            Value::String(s) => Ok(TaskIdStr(s)),
            other => Err(ConversionError::from_value(Kind::String, &other).into()),
        }
    }
}

impl Serialize for TaskIdStr {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for TaskIdStr {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        deserialize_thing_or_string(d).map(TaskIdStr)
    }
}

impl Deref for TaskIdStr {
    type Target = String;
    fn deref(&self) -> &String {
        &self.0
    }
}

impl AsRef<str> for TaskIdStr {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskIdStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl PartialEq<String> for TaskIdStr {
    fn eq(&self, other: &String) -> bool {
        self.0 == *other
    }
}

impl PartialEq<TaskIdStr> for String {
    fn eq(&self, other: &TaskIdStr) -> bool {
        *self == other.0
    }
}

/// TaskRequest entity - represents a task with rewards
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct TaskRequestEntity {
    pub id: TaskIdStr,

    // Polymorphic reference - can point to discussion or post
    pub belongs_to: RecordId,
    pub created_by: RecordId,

    pub request_txt: String,
    pub deliverable_type: DeliverableType,

    #[serde(rename = "type")]
    #[surreal(rename = "type")]
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
#[surreal(untagged)]
pub enum TaskRequestStatus {
    Init,
    InProgress,
    Completed,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, SurrealValue)]
#[surreal(untagged)]
pub enum TaskRequestType {
    Public,
    Private,
}

#[derive(Display, Clone, Debug, Serialize, Deserialize, SurrealValue)]
#[serde(tag = "type")]
#[surreal(tag = "type")]
pub enum RewardType {
    OnDelivery,
}

#[derive(EnumString, Display, Clone, Debug, Serialize, Deserialize, SurrealValue)]
#[serde(tag = "type")]
#[surreal(tag = "type")]
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
    pub id: TaskIdStr,
    pub belongs_to: RecordId,
    pub request_txt: String,
    pub currency: CurrencySymbol,
    pub donors: Vec<TaskDonorForReward>,
    pub participants: Vec<TaskParticipantForReward>,
    pub wallet: crate::entities::wallet::wallet_entity::Wallet,
    pub balance: Option<i64>,
}
