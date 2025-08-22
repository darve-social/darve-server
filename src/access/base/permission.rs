use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Permission {
    View,
    Edit,
    CreatePublicPost,
    CreatePrivatePost,
    CreateIdeaPost,
    CreatePublicTask,
    CreatePrivateTask,
    CreateDiscussion,
    AddDiscussionMember,
    RemoveDiscussionMember,
    AcceptTask,
    RejectTask,
    DeliverTask,
    CreateReply,
    Like,
    Donate,
}
