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

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscussionAccessView {
    pub id: Thing,
    pub r#type: DiscussionType,
    pub users: Vec<AccessUser>,
}

impl ViewFieldSelector for DiscussionAccessView {
    fn get_select_query_fields() -> String {
        format!("*, <-{ACCESS_TABLE_NAME}.* as users")
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PostAccessView {
    pub id: Thing,
    pub r#type: PostType,
    pub discussion: DiscussionAccessView,
    pub users: Vec<AccessUser>,
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
