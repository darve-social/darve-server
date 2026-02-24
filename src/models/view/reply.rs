use crate::{middleware::utils::db_utils::ViewFieldSelector, models::view::user::UserView};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct ReplyView {
    pub id: RecordId,
    pub belongs_to: RecordId,
    pub user: UserView,
    pub replies_nr: u32,
    pub likes_nr: u32,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub liked_by: Option<Vec<RecordId>>,
}
impl ViewFieldSelector for ReplyView {
    fn get_select_query_fields() -> String {
        "*,
         created_by.* as user,
         <-like[WHERE in=$user].in as liked_by"
            .to_string()
    }
}
