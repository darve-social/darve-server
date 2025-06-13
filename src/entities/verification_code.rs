use crate::utils::validate_utils::deserialize_thing_id;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationCode {
    #[serde(deserialize_with = "deserialize_thing_id")]
    pub id: String,
    pub code: String,
    pub failed_code_attempts: u8,
    #[serde(deserialize_with = "deserialize_thing_id")]
    pub user: String,
    pub email: String,
    pub use_for: VerificationCodeFor,
    pub r_created: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum VerificationCodeFor {
    EmailVerification,
    ResetPassword,
}
