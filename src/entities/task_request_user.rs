use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskRequestUserStatus {
    Rejected,
    Accepted,
    Delivered,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskRequestUser {
    pub id: String,
    pub task: String,
    pub user: String,
    #[serde(default)]
    pub timelines: Vec<TaskRequestUserTimeline>,
    pub result: Option<TaskRequestUserResult>,
    pub created_at: DateTime<Utc>,
}

impl TaskRequestUser {
    pub fn equal(&self, status: TaskRequestUserStatus) -> bool {
        match self.timelines.last() {
            Some(v) => v.status == status,
            None => false,
        }
    }
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
