use crate::database::table_names::ACCESS_TABLE_NAME;
use crate::entities::access_user::AccessUser;
use crate::{
    entities::community::post_entity::PostType,
    middleware::utils::db_utils::{ViewFieldSelector, ViewRelateField},
    models::view::user::UserView,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Deserialize, Serialize)]
pub struct FullPostView {
    pub id: Thing,
    pub created_by: UserView,
    pub r#type: PostType,
    pub belongs_to: Thing,
    pub title: String,
    pub content: Option<String>,
    pub media_links: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub replies_nr: i64,
    pub tasks_nr: u64,
    pub likes_nr: i64,
    pub liked_by: Option<Vec<Thing>>,
    pub users: Option<Vec<AccessUser>>,
    pub delivered_for_task: Option<Thing>,
}

impl ViewFieldSelector for FullPostView {
    fn get_select_query_fields() -> String {
        format!(
            "id,
        created_by.* as created_by, 
        title, 
        type,
        tasks_nr,
        content,
        media_links, 
        created_at,
        updated_at,
        belongs_to,
        replies_nr,
        likes_nr,
        <-{ACCESS_TABLE_NAME}.* as users,
        delivered_for_task,
        <-like[WHERE in=$user].in as liked_by"
        )
    }
}

impl ViewRelateField for FullPostView {
    fn get_fields() -> String {
        "id,
        created_by: created_by.*, 
        title, 
        type,
        tasks_nr,
        content,
        media_links, 
        created_at,
        updated_at,
        belongs_to,
        replies_nr,
        likes_nr,
        users: <-has_access.*,
        delivered_for_task,
        liked_by: <-like[WHERE in=$user].in"
            .to_string()
    }
}
