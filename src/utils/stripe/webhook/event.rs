use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Hash)]
pub enum HookEventType {
    #[serde(rename = "v2.core.account_link.completed")]
    AccountLinkCompleted,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HookEvent {
    pub id: String,
    pub created: String,
    pub livemode: bool,
    #[serde(rename = "type")]
    pub event_type: HookEventType,
}
