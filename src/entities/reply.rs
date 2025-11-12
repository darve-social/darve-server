use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Serialize, Deserialize)]
pub struct Reply {
    pub id: Thing,
    pub belongs_to: Thing,
    pub created_by: Thing,
    pub content: String,
    pub likes_nr: u32,
    pub replies_nr: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
