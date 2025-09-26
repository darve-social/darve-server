use crate::utils::validate_utils::deserialize_thing_or_string_id;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Nickname {
    #[serde(alias = "out")]
    #[serde(deserialize_with = "deserialize_thing_or_string_id")]
    user_id: String,
    name: String,
}
