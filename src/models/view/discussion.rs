use crate::database::table_names::ACCESS_TABLE_NAME;
use crate::entities::community::discussion_entity::DiscussionType;
use crate::entities::wallet::wallet_entity::UserView;
use crate::middleware::utils::db_utils::ViewFieldSelector;
use crate::models::view::access_user::AccessUserView;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
#[derive(Debug, Serialize, Deserialize)]

pub struct DiscussionView {
    pub id: Thing,
    pub r#type: DiscussionType,
    pub users: Vec<AccessUserView>,
    pub belongs_to: Thing,
    pub title: Option<String>,
    pub image_uri: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: UserView,
}

impl ViewFieldSelector for DiscussionView {
    fn get_select_query_fields() -> String {
        format!("*, created_by.* as created_by, <-{ACCESS_TABLE_NAME}.{{ user: in.*, role, created_at }} as users")
    }
}
