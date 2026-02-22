use serde::{Deserialize, Serialize};
use surrealdb::types::SurrealValue;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, SurrealValue)]
pub struct Nickname {
    pub user_id: String,
    pub name: String,
}
