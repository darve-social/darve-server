use crate::utils::validate_utils::deserialize_thing_id;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct UserNotification {
    #[serde(deserialize_with = "deserialize_thing_id")]
    pub id: String,
    #[serde(deserialize_with = "deserialize_thing_id")]
    pub created_by: String,
    #[serde(rename(deserialize = "type"))]
    pub event: UserNotificationEvent,
    pub title: String,
    #[serde(default)]
    pub is_read: bool,
    pub content: Option<String>,
    pub metadata: Option<Value>,
    pub created_at: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum UserNotificationEvent {
    DiscussionPostAdded,
    DiscussionPostReplyAdded,
    DiscussionPostReplyNrIncreased,
    UserBalanceUpdate,
    UserChatMessage,
    UserCommunityPost,
    UserFollowAdded,
    UserLikePost,
    UserTaskRequestCreated,
    UserTaskRequestDelivered,
    UserTaskRequestReceived,
}

impl UserNotificationEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserNotificationEvent::UserFollowAdded => "UserFollowAdded",
            UserNotificationEvent::UserTaskRequestCreated => "UserTaskRequestCreated",
            UserNotificationEvent::UserTaskRequestReceived => "UserTaskRequestReceived",
            UserNotificationEvent::UserTaskRequestDelivered => "UserTaskRequestDelivered",
            UserNotificationEvent::UserChatMessage => "UserChatMessage",
            UserNotificationEvent::UserCommunityPost => "UserCommunityPost",
            UserNotificationEvent::UserBalanceUpdate => "UserBalanceUpdate",
            UserNotificationEvent::UserLikePost => "UserLikePost",
            UserNotificationEvent::DiscussionPostReplyAdded => "DiscussionPostReplyAdded",
            UserNotificationEvent::DiscussionPostReplyNrIncreased => {
                "DiscussionPostReplyNrIncreased"
            }
            UserNotificationEvent::DiscussionPostAdded => "DiscussionPostAdded",
        }
    }
}
