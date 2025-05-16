use middleware::{
    ctx::Ctx,
    db,
    error::{AppError, CtxError, CtxResult},
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::{
    entities::{community::post_entity, user_auth::local_user_entity},
    middleware,
};

use super::task_request_entity;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskDeliverable {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub user: Thing,
    pub task_request: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Thing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

pub struct TaskDeliverableDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

impl<'a> TaskDeliverableDbService<'a> {}

pub const TABLE_NAME: &str = "task_deliverable";
const TABLE_COL_USER: &str = local_user_entity::TABLE_NAME;
const TABLE_COL_POST: &str = post_entity::TABLE_NAME;
const TABLE_COL_TASK_REQ: &str = task_request_entity::TABLE_NAME;

impl<'a> TaskDeliverableDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS user ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE FIELD IF NOT EXISTS task_request ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_TASK_REQ}>;
    DEFINE FIELD IF NOT EXISTS urls ON TABLE {TABLE_NAME} TYPE option<set<string>>;
    DEFINE FIELD IF NOT EXISTS post ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_COL_POST}>>;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    //DEFINE INDEX IF NOT EXISTS r_created_idx ON TABLE {TABLE_NAME} COLUMNS r_created;
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate TaskDeliverable");

        Ok(())
    }

    pub async fn create(&self, record: TaskDeliverable) -> CtxResult<TaskDeliverable> {
        let res = self
            .db
            .create(TABLE_NAME)
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<TaskDeliverable>| v.unwrap());

        // let things: Vec<Domain> = self.db.select(TABLE_NAME).await.ok().unwrap();
        // dbg!(things);
        res
    }

    /*pub async fn get(&self, ident: IdentIdName) -> CtxResult<TaskDeliverable> {
        let opt = get_entity::<TaskDeliverable>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }*/
}
