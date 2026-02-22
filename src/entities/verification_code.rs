use crate::database::repository_traits::EntityWithId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

// TODO this id macros are db dependant so maybe can use some surrealdb build flag that adds id macros so we don't need separate struct definition for db and service,route
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct VerificationCodeEntity {
    pub id: Option<RecordId>,
    pub code: String,
    pub failed_code_attempts: u8,
    pub user: RecordId,
    pub email: String,
    pub use_for: VerificationCodeFor,
    pub r_created: DateTime<Utc>,
}

impl EntityWithId for VerificationCodeEntity {
    fn id_str(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SurrealValue)]
#[surreal(untagged)]
pub enum VerificationCodeFor {
    EmailVerification,
    ResetPassword,
    SetPassword,
    UpdatePassword,
}
