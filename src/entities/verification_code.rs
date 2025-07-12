use crate::database::repositories::verification_code_repo::VERIFICATION_CODE_TABLE_NAME;
use crate::database::repository::OptionalIdentifier;
use crate::utils::validate_utils::{deserialize_thing_id, serialize_string_id};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

// TODO this id macros are db dependant so maybe can use some surrealdb build flag that adds id macros so we don't need separate struct definition for db and service,route
#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationCodeEntity {
    #[serde(deserialize_with = "deserialize_thing_id")]
    #[serde(serialize_with = "serialize_string_id")]
    pub id: String,
    pub code: String,
    pub failed_code_attempts: u8,
    #[serde(deserialize_with = "deserialize_thing_id")]
    pub user: String,
    pub email: String,
    pub use_for: VerificationCodeFor,
    pub r_created: DateTime<Utc>,
}

impl OptionalIdentifier for VerificationCodeEntity {
    fn ident_ref(&self) -> Option<Thing> {
        match self.id.is_empty() {
            true => Some(Thing::from((VERIFICATION_CODE_TABLE_NAME, self.id.as_ref()))),
            false => None
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum VerificationCodeFor {
    EmailVerification,
    ResetPassword,
}
