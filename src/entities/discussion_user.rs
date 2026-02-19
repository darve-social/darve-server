use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct DiscussionUser {
    #[serde(alias = "out")]
    pub user: RecordId,
    #[serde(alias = "in")]
    pub discussion: RecordId,
    pub latest_post: Option<RecordId>,
    pub nr_unread: u32,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
