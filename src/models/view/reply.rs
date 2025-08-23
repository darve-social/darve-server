use crate::{middleware::utils::db_utils::ViewFieldSelector, models::view::user::UserView};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplyView {
    pub id: Thing,
    pub user: UserView,
    pub likes_nr: u32,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl ViewFieldSelector for ReplyView {
    fn get_select_query_fields() -> String {
        "id, content, likes_nr, created_at, updated_at, created_by.* as user".to_string()
    }
}
