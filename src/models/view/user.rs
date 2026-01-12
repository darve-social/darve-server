use crate::{
    entities::user_auth::local_user_entity::{LocalUser, UserRole},
    middleware::utils::db_utils::ViewFieldSelector,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Serialize, Deserialize)]
pub struct UserView {
    pub id: Thing,
    pub username: String,
    pub full_name: Option<String>,
    pub birth_date: Option<String>,
    pub phone: Option<String>,
    pub email_verified: Option<String>,
    pub bio: Option<String>,
    pub image_uri: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
    pub role: UserRole,
}

impl From<LocalUser> for UserView {
    fn from(user: LocalUser) -> Self {
        UserView {
            id: user.id.unwrap().clone(),
            username: user.username,
            full_name: user.full_name,
            birth_date: user.birth_date,
            phone: user.phone,
            email_verified: user.email_verified,
            bio: user.bio,
            image_uri: user.image_uri,
            last_seen: user.last_seen,
            role: user.role,
        }
    }
}

impl ViewFieldSelector for UserView {
    fn get_select_query_fields() -> String {
        "*".to_string()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoggedUserView {
    pub id: Thing,
    pub username: String,
    pub full_name: Option<String>,
    pub birth_date: Option<String>,
    pub phone: Option<String>,
    pub email_verified: Option<String>,
    pub bio: Option<String>,
    pub social_links: Option<Vec<String>>,
    pub image_uri: Option<String>,
    pub is_otp_enabled: bool,
    pub last_seen: Option<DateTime<Utc>>,
    pub has_password: bool,
    pub role: UserRole,
}

impl From<(LocalUser, bool)> for LoggedUserView {
    fn from((user, has_password): (LocalUser, bool)) -> Self {
        LoggedUserView {
            id: user.id.unwrap().clone(),
            username: user.username,
            full_name: user.full_name,
            birth_date: user.birth_date,
            phone: user.phone,
            email_verified: user.email_verified,
            bio: user.bio,
            social_links: user.social_links,
            image_uri: user.image_uri,
            is_otp_enabled: user.is_otp_enabled,
            last_seen: user.last_seen,
            has_password: has_password,
            role: user.role,
        }
    }
}

impl ViewFieldSelector for LoggedUserView {
    fn get_select_query_fields() -> String {
        "*".to_string()
    }
}
