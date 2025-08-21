use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::view::user::UserView;

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessUserView {
    pub role: String,
    #[serde(rename = "in")]
    pub user: UserView,
    pub created_at: DateTime<Utc>,
}
