use crate::database::table_names::DISC_USER_TABLE_NAME;
use crate::entities::community::discussion_entity::DiscussionType;
use crate::entities::wallet::wallet_entity::UserView;
use crate::middleware::utils::db_utils::ViewFieldSelector;
use crate::models::view::access_user::AccessUserView;
use crate::{
    database::table_names::ACCESS_TABLE_NAME, middleware::utils::db_utils::ViewRelateField,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
#[derive(Debug, Serialize, Deserialize, SurrealValue)]

pub struct DiscussionView {
    pub id: RecordId,
    #[surreal(rename = "type")]
    pub r#type: DiscussionType,
    pub users: Vec<AccessUserView>,
    pub belongs_to: RecordId,
    pub title: Option<String>,
    pub image_uri: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: UserView,
    pub alias: Option<String>,
}

impl ViewFieldSelector for DiscussionView {
    fn get_select_query_fields() -> String {
        format!(
            "*,
            created_by.* as created_by,
            <-{ACCESS_TABLE_NAME}.{{ user: in.*, role, created_at }} as users,
            ->{DISC_USER_TABLE_NAME}[WHERE out=$user][0].alias AS alias"
        )
    }
}

impl ViewRelateField for DiscussionView {
    fn get_fields() -> String {
        "id,
        type,
        belongs_to,
        title,
        image_uri,
        created_at,
        updated_at,
        created_by: created_by.*,
        users: <-has_access.{user: in.*, role, created_at},
        alias: ->discussion_user[WHERE out=$user][0].alias"
            .to_string()
    }
}
