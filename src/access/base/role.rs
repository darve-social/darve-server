use serde::Deserialize;
use std::fmt::Display;

#[derive(Debug, Deserialize, Eq, PartialEq, Hash, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum Role {
    Guest,
    Admin,
    Member,
    Owner,
    Donor,
    Candidate,
    Participant,
    Editor,
}

impl Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Guest => write!(f, "GUEST"),
            Role::Member => write!(f, "MEMBER"),
            Role::Owner => write!(f, "OWNER"),
            Role::Candidate => write!(f, "CANDIDATE"),
            Role::Participant => write!(f, "PARTICIPANT"),
            Role::Donor => write!(f, "DONOR"),
            Role::Editor => write!(f, "EDITOR"),
            Role::Admin => write!(f, "ADMIN"),
        }
    }
}

impl From<&str> for Role {
    fn from(value: &str) -> Self {
        match value {
            "GUEST" => Role::Guest,
            "MEMBER" => Role::Member,
            "OWNER" => Role::Owner,
            "CANDIDATE" => Role::Candidate,
            "PARTICIPANT" => Role::Participant,
            "DONOR" => Role::Donor,
            "EDITOR" => Role::Editor,
            "ADMIN" => Role::Admin,
            _ => panic!("Unknown role: {}", value),
        }
    }
}
