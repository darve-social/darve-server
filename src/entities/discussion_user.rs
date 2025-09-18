use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiscussionUser {
    #[serde(alias = "out")]
    pub user: Thing,
    #[serde(alias = "in")]
    pub discussion: Thing,
    pub latest_post: Option<Thing>,
    pub nr_unread: u32,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
