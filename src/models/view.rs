use crate::{
    entities::user_auth::local_user_entity::LocalUser,
    middleware::utils::db_utils::{ViewFieldSelector, ViewRelateField},
};
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
    pub social_links: Option<Vec<String>>,
    pub image_uri: Option<String>,
    pub is_otp_enabled: bool,
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
            social_links: user.social_links,
            image_uri: user.image_uri,
            is_otp_enabled: user.is_otp_enabled,
        }
    }
}

impl ViewFieldSelector for UserView {
    fn get_select_query_fields() -> String {
        "*".to_string()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PostView {
    pub id: Thing,
    pub created_by_name: String,
    pub belongs_to_uri: Option<String>,
    pub belongs_to_id: Thing,
    pub title: String,
    pub r_title_uri: Option<String>,
    pub content: String,
    pub media_links: Option<Vec<String>>,
    pub r_created: String,
    pub replies_nr: i64,
    pub likes_nr: i64,
}

impl ViewFieldSelector for PostView {
    // post fields selct qry for view
    fn get_select_query_fields() -> String {
        "id,
         created_by.username as created_by_name,
         title,
         r_title_uri,
         content,
         media_links,
         r_created,
         belongs_to.name_uri as belongs_to_uri,
         belongs_to.id as belongs_to_id,
         replies_nr,
         likes_nr"
            .to_string()
    }
}

impl ViewRelateField for PostView {
    fn get_fields() -> &'static str {
        "id,
        created_by_name: created_by.username, 
        title, 
        r_title_uri, 
        content,
        media_links, 
        r_created, 
        belongs_to_uri: belongs_to.name_uri, 
        belongs_to_id: belongs_to.id,
        replies_nr,
        likes_nr"
    }
}
