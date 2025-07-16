use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    pub id: String,
    pub task: String,
    pub user: String,
    pub status: TaskParticipantStatus,
    #[serde(default)]
    pub timelines: Vec<TaskParticipantTimeline>,
    pub result: Option<TaskParticipantResult>,
    pub reward_tx: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskParticipantTimeline {
    pub status: TaskParticipantStatus,
    pub date: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskParticipantResult {
    pub urls: Option<Vec<String>>,
    pub post: Option<String>,
}
