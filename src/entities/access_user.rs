use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct AccessUser {
    pub role: String,
    #[serde(alias = "in")]
    pub user: RecordId,
    pub created_at: DateTime<Utc>,
}
