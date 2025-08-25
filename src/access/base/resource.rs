use serde::Deserialize;
use std::fmt::Display;

#[derive(Debug, Deserialize, Eq, PartialEq, Hash, Clone)]
pub enum Resource {
    #[serde(alias = "APP")]
    App,
    #[serde(alias = "DISCUSSION:PUBLIC")]
    DiscussionPublic,
    #[serde(alias = "DISCUSSION:PRIVATE")]
    DiscussionPrivate,
    #[serde(alias = "TASK:PRIVATE")]
    TaskPrivate,
    #[serde(alias = "TASK:PUBLIC")]
    TaskPublic,
    #[serde(alias = "POST:PUBLIC")]
    PostPublic,
    #[serde(alias = "POST:PRIVATE")]
    PostPrivate,
    #[serde(alias = "POST:IDEA")]
    PostIdea,
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
            Resource::PostIdea => write!(f, "POST:IDEA"),
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
            "POST:IDEA" => Resource::PostIdea,
            _ => panic!("Unknown resource: {}", value),
        }
    }
}
