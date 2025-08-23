use serde::Deserialize;
use std::fmt::Display;

#[derive(Debug, Deserialize, Eq, PartialEq, Hash, Clone)]
pub enum Role {
    Guest,
    Member,
    Owner,
    Donor,
    Candidate,
    Participant,
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
            _ => panic!("Unknown role: {}", value),
        }
    }
}
