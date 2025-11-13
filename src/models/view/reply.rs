use crate::{middleware::utils::db_utils::ViewFieldSelector, models::view::user::UserView};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplyView {
    pub id: Thing,
    pub belongs_to: Thing,
    pub user: UserView,
    pub replies_nr: u32,
    pub likes_nr: u32,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub liked_by: Option<Vec<Thing>>,
}
impl ViewFieldSelector for ReplyView {
    fn get_select_query_fields() -> String {
        "*,
         created_by.* as user,
         <-like[WHERE in=$user].in as liked_by"
            .to_string()
    }
}
