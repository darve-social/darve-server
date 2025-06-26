use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::sql::Datetime;

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct UserNotification {
    pub id: String,
    pub created_by: String,
    pub event: UserNotificationEvent,
    pub title: String,
    #[serde(default)]
    pub is_read: bool,
    // pub content: Option<String>,
    pub metadata: Option<Value>,
    pub created_at: Datetime,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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
