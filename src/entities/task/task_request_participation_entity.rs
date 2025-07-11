use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequestParticipation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub(crate) user: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transaction: Option<Thing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) votes: Option<Vec<RewardVote>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RewardVote {
    deliverable_ident: String,
    points: i32,
}

pub const TABLE_NAME: &str = "task_request_participation";
