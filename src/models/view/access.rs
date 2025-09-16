use crate::database::table_names::ACCESS_TABLE_NAME;
use crate::entities::community::post_entity::PostType;
use crate::entities::community::{
    discussion_entity::TABLE_NAME as DISC_TABLE_NAME, post_entity::TABLE_NAME as POST_TABLE_NAME,
};
use crate::entities::task::task_request_entity::TaskRequestType;
use crate::entities::{access_user::AccessUser, community::discussion_entity::DiscussionType};
use crate::middleware::utils::db_utils::ViewFieldSelector;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiscussionAccessView {
    pub id: Thing,
    pub r#type: DiscussionType,
    pub users: Vec<AccessUser>,
}

impl DiscussionAccessView {
    pub fn get_user_ids(&self) -> Vec<Thing> {
        self.users
            .iter()
            .map(|user| user.user.clone())
            .collect::<Vec<Thing>>()
    }

    pub fn get_by_role(&self, role: &str) -> Vec<Thing> {
        self.users
            .iter()
            .filter(|u| u.role == role)
            .map(|u| u.user.clone())
            .collect::<Vec<Thing>>()
    }
}

impl ViewFieldSelector for DiscussionAccessView {
    fn get_select_query_fields() -> String {
        format!("*, <-{ACCESS_TABLE_NAME}.* as users")
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PostAccessView {
    pub id: Thing,
    pub r#type: PostType,
    pub discussion: DiscussionAccessView,
    pub users: Vec<AccessUser>,
}

impl PostAccessView {
    pub fn get_user_ids(&self) -> Vec<Thing> {
        self.users
            .iter()
            .map(|user| user.user.clone())
            .collect::<Vec<Thing>>()
    }

    pub fn get_by_role(&self, role: &str) -> Vec<Thing> {
        self.users
            .iter()
            .filter(|u| u.role == role)
            .map(|u| u.user.clone())
            .collect::<Vec<Thing>>()
    }
}

impl ViewFieldSelector for PostAccessView {
    fn get_select_query_fields() -> String {
        format!("*, <-{ACCESS_TABLE_NAME}.* as users, belongs_to.{{ id, type, users: <-{ACCESS_TABLE_NAME}.*}} as discussion")
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TaskAccessView {
    pub id: Thing,
    pub r#type: TaskRequestType,
    pub post: Option<PostAccessView>,
    pub discussion: Option<DiscussionAccessView>,
    pub users: Vec<AccessUser>,
}
impl TaskAccessView {
    pub fn get_user_ids(&self) -> Vec<Thing> {
        self.users
            .iter()
            .map(|user| user.user.clone())
            .collect::<Vec<Thing>>()
    }

    pub fn get_by_role(&self, role: &str) -> Vec<Thing> {
        self.users
            .iter()
            .filter(|u| u.role == role)
            .map(|u| u.user.clone())
            .collect::<Vec<Thing>>()
    }
}

impl ViewFieldSelector for TaskAccessView {
    fn get_select_query_fields() -> String {
        format!(
            "id, type, <-{ACCESS_TABLE_NAME}.* as users, 
                IF record::tb(belongs_to) = '{POST_TABLE_NAME}' THEN belongs_to.{{ 
                        id, 
                        type, 
                        users: <-{ACCESS_TABLE_NAME}.*, 
                        discussion: belongs_to.{{ 
                            id, 
                            type, 
                            users: <-{ACCESS_TABLE_NAME}.* 
                     }} 
                }} END AS post,
                IF record::tb(belongs_to) = '{DISC_TABLE_NAME}' THEN belongs_to.{{ 
                        id, 
                        type, 
                        users: <-{ACCESS_TABLE_NAME}.* 
                }} END AS discussion"
        )
    }
}
