use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::utils::validate_utils::deserialize_thing_or_string_id;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskParticipantStatus {
    Requested,
    Rejected,
    Accepted,
    Delivered,
}

impl TaskParticipantStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskParticipantStatus::Requested => "Requested",
            TaskParticipantStatus::Rejected => "Rejected",
            TaskParticipantStatus::Accepted => "Accepted",
            TaskParticipantStatus::Delivered => "Delivered",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskParticipant {
    #[serde(deserialize_with = "deserialize_thing_or_string_id")]
    pub id: String,
    #[serde(alias = "in")]
    #[serde(deserialize_with = "deserialize_thing_or_string_id")]
    pub task: String,
    #[serde(alias = "out")]
    #[serde(deserialize_with = "deserialize_thing_or_string_id")]
    pub user: String,
    pub status: TaskParticipantStatus,
    #[serde(default)]
    pub timelines: Vec<TaskParticipantTimeline>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskParticipantTimeline {
    pub status: TaskParticipantStatus,
    pub date: DateTime<Utc>,
}
