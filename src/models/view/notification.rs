use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    entities::user_notification::UserNotificationEvent,
    middleware::utils::db_utils::ViewRelateField, models::view::user::UserView,
    utils::validate_utils::deserialize_thing_or_string_id,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct UserNotificationView {
    #[serde(deserialize_with = "deserialize_thing_or_string_id")]
    pub id: String,
    pub created_by: UserView,
    pub event: UserNotificationEvent,
    pub title: String,
    #[serde(default)]
    pub is_read: bool,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
}

impl ViewRelateField for UserNotificationView {
    fn get_fields() -> &'static str {
        "id, created_by: created_by.*, title, event, is_read, metadata, created_at"
    }
}
