use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::database::repository::OptionalIdentifier;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskDeliverable {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub user: Thing,
    pub task_request: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Thing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

impl OptionalIdentifier for TaskDeliverable {
    fn ident_ref(&self) -> Option<&Thing> {
        self.id.as_ref()
    }
}

pub const TABLE_NAME: &str = "task_deliverable";