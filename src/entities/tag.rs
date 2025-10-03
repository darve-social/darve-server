use crate::utils::validate_utils::deserialize_thing_or_string_id;
use serde::{Deserialize, Serialize};

pub enum SystemTags {
    Delivery,
}

impl SystemTags {
    pub fn as_str(&self) -> &'static str {
        match self {
            SystemTags::Delivery => "_delivery",
        }
    }
}

impl TryFrom<&str> for SystemTags {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "_delivery" => Ok(SystemTags::Delivery),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Tag {
    #[serde(alias = "id")]
    #[serde(deserialize_with = "deserialize_thing_or_string_id")]
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EditorTag {
    #[serde(alias = "tag")]
    #[serde(deserialize_with = "deserialize_thing_or_string_id")]
    pub name: String,
    pub image_url: String,
}
