use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskRequestUserStatus {
    Requested,
    Rejected,
    Accepted,
    Delivered,
}

impl TaskRequestUserStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskRequestUserStatus::Requested => "Requested",
            TaskRequestUserStatus::Rejected => "Rejected",
            TaskRequestUserStatus::Accepted => "Accepted",
            TaskRequestUserStatus::Delivered => "Delivered",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskRequestUser {
    pub id: String,
    pub task: String,
    pub user: String,
    pub status: TaskRequestUserStatus,
    #[serde(default)]
    pub timelines: Vec<TaskRequestUserTimeline>,
    pub result: Option<TaskRequestUserResult>,
    pub reward_tx: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskRequestUserTimeline {
    pub status: TaskRequestUserStatus,
    pub date: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskRequestUserResult {
    pub urls: Option<Vec<String>>,
    pub post: Option<String>,
}
