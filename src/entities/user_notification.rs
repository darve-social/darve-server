use crate::utils::validate_utils::deserialize_thing_or_string_id;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct UserNotification {
    #[serde(deserialize_with = "deserialize_thing_or_string_id")]
    pub id: String,
    #[serde(deserialize_with = "deserialize_thing_or_string_id")]
    pub created_by: String,
    pub event: UserNotificationEvent,
    pub title: String,
    #[serde(default)]
    pub is_read: bool,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum UserNotificationEvent {
    UserBalanceUpdate,
    UserFollowAdded,
    UserLikePost,
    UserTaskRequestCreated,
    UserTaskRequestDelivered,
    UserTaskRequestReceived,
    DonateTaskRequest,
    UserTaskRequestAccepted,
    TaskRewardReceived,
    CreatedPost,
    CommentAdded,
    UserLikeComment,
    DepositCompleted,
    WithdrawCompleted,
    CreatedDiscussion,
}

impl UserNotificationEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserNotificationEvent::UserFollowAdded => "UserFollowAdded",
            UserNotificationEvent::UserTaskRequestCreated => "UserTaskRequestCreated",
            UserNotificationEvent::UserTaskRequestDelivered => "UserTaskRequestDelivered",
            UserNotificationEvent::UserTaskRequestReceived => "UserTaskRequestReceived",
            UserNotificationEvent::UserTaskRequestAccepted => "UserTaskRequestAccepted",
            UserNotificationEvent::DonateTaskRequest => "DonateTaskRequest",
            UserNotificationEvent::TaskRewardReceived => "TaskRewardReceived",
            UserNotificationEvent::CreatedPost => "CreatedPost",
            UserNotificationEvent::UserBalanceUpdate => "UserBalanceUpdate",
            UserNotificationEvent::UserLikePost => "UserLikePost",
            UserNotificationEvent::CommentAdded => "CommentAdded",
            UserNotificationEvent::UserLikeComment => "UserLikeComment",
            UserNotificationEvent::DepositCompleted => "DepositCompleted",
            UserNotificationEvent::WithdrawCompleted => "WithdrawCompleted",
            UserNotificationEvent::CreatedDiscussion => "CreatedDiscussion",
        }
    }
}
