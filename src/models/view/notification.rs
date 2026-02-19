use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::types::SurrealValue;

use crate::{
    entities::user_notification::UserNotificationEvent,
    middleware::utils::db_utils::ViewRelateField, models::view::user::UserView,
    utils::validate_utils::deserialize_thing_or_string_id,
};

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
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
    #[serde(default)]
    pub is_follower: bool,
    #[serde(default)]
    pub is_following: bool,
}

impl ViewRelateField for UserNotificationView {
    fn get_fields() -> String {
        "id, is_following: created_by IN $user->follow.out, is_follower: created_by IN $user<-follow.in, created_by: created_by.*, title, event, is_read, metadata, created_at".to_string()
    }
}
