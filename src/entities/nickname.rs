use crate::utils::validate_utils::deserialize_thing_or_string_id;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Nickname {
    #[serde(alias = "out")]
    #[serde(deserialize_with = "deserialize_thing_or_string_id")]
    pub user_id: String,
    pub name: String,
}
