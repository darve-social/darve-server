use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct Reply {
    pub id: RecordId,
    pub belongs_to: RecordId,
    pub created_by: RecordId,
    pub content: String,
    pub likes_nr: u32,
    pub replies_nr: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
