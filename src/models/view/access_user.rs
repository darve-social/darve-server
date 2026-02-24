use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::SurrealValue;

use crate::models::view::user::UserView;

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct AccessUserView {
    pub role: String,
    pub user: UserView,
    pub created_at: DateTime<Utc>,
}
