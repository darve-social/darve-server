use crate::database::table_names::ACCESS_TABLE_NAME;
use crate::database::table_names::POST_USER_TABLE_NAME;
use crate::entities::access_user::AccessUser;
use crate::entities::community::post_entity::PostUserStatus;
use crate::models::view::access_user::AccessUserView;
use crate::{
    entities::community::post_entity::PostType,
    middleware::utils::db_utils::{ViewFieldSelector, ViewRelateField},
    models::view::user::UserView,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
#[derive(Debug, Deserialize, Serialize)]
pub struct PostView {
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
    pub reply_to: Option<Box<PostView>>,
}

impl ViewFieldSelector for PostView {
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
        <-like[WHERE in=$user].in as liked_by,
        reply_to.{{id, created_by: created_by.*, title, type, tasks_nr, content, media_links, created_at, updated_at, belongs_to, replies_nr, likes_nr}} as reply_to"
        )
    }
}

impl ViewRelateField for PostView {
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
        liked_by: <-like[WHERE in=$user].in,
        reply_to: reply_to.{
            id, created_by: created_by.*, title, type, tasks_nr, content, media_links, created_at, updated_at, belongs_to, replies_nr, likes_nr}"
            .to_string()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PostUsersView {
    pub id: Thing,
    pub users: Option<Vec<AccessUserView>>,
}

impl ViewFieldSelector for PostUsersView {
    fn get_select_query_fields() -> String {
        format!("id, <-{ACCESS_TABLE_NAME}.{{ user: in.*, role, created_at }} as users")
    }
}

impl ViewRelateField for PostUsersView {
    fn get_fields() -> String {
        "id, users: <-has_access.{ user: in.*, role, created_at }".to_string()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PostUserStatusView {
    pub status: PostUserStatus,
    #[serde(rename = "out")]
    pub user: Thing,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LatestPostView {
    pub id: Thing,
    pub created_by: UserView,
    pub r#type: PostType,
    pub belongs_to: Thing,
    pub title: String,
    pub content: Option<String>,
    pub media_links: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub users_status: Option<Vec<PostUserStatusView>>,
}

impl ViewFieldSelector for LatestPostView {
    fn get_select_query_fields() -> String {
        format!(
            "id,
        created_by.* as created_by, 
        title, 
        type,
        content,
        media_links, 
        created_at,
        updated_at,
        belongs_to,
        ->{POST_USER_TABLE_NAME}.* as users_status"
        )
    }
}

impl ViewRelateField for LatestPostView {
    fn get_fields() -> String {
        "id,
        created_by: created_by.*, 
        title, 
        type,
        content,
        media_links, 
        created_at,
        updated_at,
        belongs_to,
        users_status: ->post_user.*"
            .to_string()
    }
}
