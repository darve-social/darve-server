use crate::entities::community::post_entity;
use crate::entities::task::task_deliverable_entity::TaskDeliverable;
use crate::entities::task::task_request_entity;
use crate::entities::user_auth::local_user_entity;
use crate::{
    database::repository::Repository,
    middleware::error::AppError,
};

const TABLE_COL_USER: &str = local_user_entity::TABLE_NAME;
const TABLE_COL_POST: &str = post_entity::TABLE_NAME;
const TABLE_COL_TASK_REQ: &str = task_request_entity::TABLE_NAME;

impl Repository<TaskDeliverable> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let table_name = self.table_name.as_str();
        
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {table_name} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS user ON TABLE {table_name} TYPE record<{TABLE_COL_USER}>;
    DEFINE FIELD IF NOT EXISTS task_request ON TABLE {table_name} TYPE record<{TABLE_COL_TASK_REQ}>;
    DEFINE FIELD IF NOT EXISTS urls ON TABLE {table_name} TYPE option<set<string>>;
    DEFINE FIELD IF NOT EXISTS post ON TABLE {table_name} TYPE option<record<{TABLE_COL_POST}>>;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {table_name} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    //DEFINE INDEX IF NOT EXISTS r_created_idx ON TABLE {table_name} COLUMNS r_created;
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {table_name} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.client.query(sql).await?;
        mutation.check().expect("should mutate TaskDeliverable");

        Ok(())
    }
}