use serde::Deserialize;
use std::fmt::Display;

#[derive(Debug, Deserialize, Eq, PartialEq, Hash, Clone)]
pub enum Resource {
    App,
    DiscussionPublic,
    DiscussionPrivate,
    TaskPrivate,
    TaskPublic,
    PostPublic,
    PostPrivate,
}

impl Display for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Resource::App => write!(f, "APP"),
            Resource::DiscussionPublic => write!(f, "DISCUSSION:PUBLIC"),
            Resource::DiscussionPrivate => write!(f, "DISCUSSION:PRIVATE"),
            Resource::TaskPrivate => write!(f, "TASK:PRIVATE"),
            Resource::TaskPublic => write!(f, "TASK:PUBLIC"),
            Resource::PostPublic => write!(f, "POST:PUBLIC"),
            Resource::PostPrivate => write!(f, "POST:PRIVATE"),
        }
    }
}

impl From<&str> for Resource {
    fn from(value: &str) -> Self {
        match value {
            "APP" => Resource::App,
            "DISCUSSION:PUBLIC" => Resource::DiscussionPublic,
            "DISCUSSION:PRIVATE" => Resource::DiscussionPrivate,
            "TASK:PRIVATE" => Resource::TaskPrivate,
            "TASK:PUBLIC" => Resource::TaskPublic,
            "POST:PUBLIC" => Resource::PostPublic,
            "POST:PRIVATE" => Resource::PostPrivate,
            _ => panic!("Unknown resource: {}", value),
        }
    }
}
