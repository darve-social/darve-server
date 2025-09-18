use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscussionUser {
    #[serde(alias = "out")]
    user: Thing,
    #[serde(alias = "in")]
    discussion: Thing,
    latest_post: Option<Thing>,
    nr_unread: u32,
    updated_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}
