use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessUser {
    pub role: String,
    #[serde(alias = "in")]
    pub user: Thing,
    pub created_at: DateTime<Utc>,
}
