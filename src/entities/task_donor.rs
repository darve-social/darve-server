use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Clone, Debug, Serialize, Deserialize, SurrealValue)]
pub struct TaskDonor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RecordId>,
    #[serde(alias = "out")]
    pub(crate) user: RecordId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transaction: Option<RecordId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) votes: Option<Vec<RewardVote>>,
    pub(crate) amount: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, SurrealValue)]
pub struct RewardVote {
    deliverable_ident: String,
    points: i32,
}
