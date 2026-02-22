use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::types::SurrealValue;

use crate::{
    entities::user_notification::UserNotificationEvent,
    middleware::utils::db_utils::ViewRelateField, models::view::user::UserView,
};

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct UserNotificationView {
    pub id: String,
    pub created_by: UserView,
    pub event: UserNotificationEvent,
    pub title: String,
    pub is_read: bool,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub is_follower: bool,
    pub is_following: bool,
}

impl ViewRelateField for UserNotificationView {
    fn get_fields() -> String {
        "record::id(out.id) as id, (out.created_by IN $user->follow.out) ?? false as is_following, (out.created_by IN $user<-follow.in) ?? false as is_follower, out.created_by.{id, username, full_name, birth_date, phone, email_verified, bio, image_uri, last_seen, role} as created_by, out.title as title, out.event as event, is_read ?? false as is_read, out.metadata as metadata, out.created_at as created_at".to_string()
    }
}
