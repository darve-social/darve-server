use crate::database::table_names::ACCESS_TABLE_NAME;
use crate::entities::community::post_entity::PostType;
use crate::entities::community::{
    discussion_entity::TABLE_NAME as DISC_TABLE_NAME, post_entity::TABLE_NAME as POST_TABLE_NAME,
};
use crate::entities::task_request::TaskRequestType;
use crate::entities::{access_user::AccessUser, community::discussion_entity::DiscussionType};
use crate::middleware::utils::db_utils::{ViewFieldSelector, ViewRelateField};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct DiscussionAccessView {
    pub id: RecordId,
    #[surreal(rename = "type")]
    pub r#type: DiscussionType,
    pub created_by: RecordId,
    pub users: Vec<AccessUser>,
}

impl DiscussionAccessView {
    pub fn get_user_ids(&self) -> Vec<RecordId> {
        self.users
            .iter()
            .map(|user| user.user.clone())
            .collect::<Vec<RecordId>>()
    }

    pub fn get_by_role(&self, role: &str) -> Vec<RecordId> {
        self.users
            .iter()
            .filter(|u| u.role == role)
            .map(|u| u.user.clone())
            .collect::<Vec<RecordId>>()
    }
}

impl ViewFieldSelector for DiscussionAccessView {
    fn get_select_query_fields() -> String {
        format!("id, type, created_by, <-{ACCESS_TABLE_NAME}.* as users")
    }
}

impl ViewRelateField for DiscussionAccessView {
    fn get_fields() -> String {
        "id, type, created_by, users: <-has_access.*".to_string()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, SurrealValue)]
pub struct PostAccessView {
    pub id: RecordId,
    #[surreal(rename = "type")]
    pub r#type: PostType,
    pub discussion: DiscussionAccessView,
    pub users: Vec<AccessUser>,
    pub media_links: Option<Vec<String>>,
    pub tasks_nr: u32,
}

impl PostAccessView {
    pub fn get_user_ids(&self) -> Vec<RecordId> {
        self.users
            .iter()
            .map(|user| user.user.clone())
            .collect::<Vec<RecordId>>()
    }

    pub fn get_by_role(&self, role: &str) -> Vec<RecordId> {
        self.users
            .iter()
            .filter(|u| u.role == role)
            .map(|u| u.user.clone())
            .collect::<Vec<RecordId>>()
    }
}

impl ViewFieldSelector for PostAccessView {
    fn get_select_query_fields() -> String {
        let disc_fields = DiscussionAccessView::get_fields();
        format!("*, <-{ACCESS_TABLE_NAME}.* as users, belongs_to.{{{disc_fields}}} as discussion")
    }
}

#[derive(Debug, Deserialize, Serialize, SurrealValue)]
pub struct TaskAccessView {
    pub id: RecordId,
    #[surreal(rename = "type")]
    pub r#type: TaskRequestType,
    pub post: Option<PostAccessView>,
    pub discussion: Option<DiscussionAccessView>,
    pub users: Vec<AccessUser>,
}
impl TaskAccessView {
    pub fn get_user_ids(&self) -> Vec<RecordId> {
        self.users
            .iter()
            .map(|user| user.user.clone())
            .collect::<Vec<RecordId>>()
    }

    pub fn get_by_role(&self, role: &str) -> Vec<RecordId> {
        self.users
            .iter()
            .filter(|u| u.role == role)
            .map(|u| u.user.clone())
            .collect::<Vec<RecordId>>()
    }
}

impl ViewFieldSelector for TaskAccessView {
    fn get_select_query_fields() -> String {
        let disc_fields = DiscussionAccessView::get_fields();
        format!(
            "id, type, <-{ACCESS_TABLE_NAME}.* as users,
                IF record::tb(belongs_to) = '{POST_TABLE_NAME}' THEN belongs_to.{{
                        id,
                        type,
                        tasks_nr,
                        media_links,
                        users: <-{ACCESS_TABLE_NAME}.*,
                        discussion: belongs_to.{{{disc_fields}}}
                }} END AS post,
                IF record::tb(belongs_to) = '{DISC_TABLE_NAME}' THEN belongs_to.{{{disc_fields}}} END AS discussion"
        )
    }
}
