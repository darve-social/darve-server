use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use sb_middleware::{
    ctx::Ctx,
    error::{CtxError, CtxResult, AppError},
};
use sb_middleware::db;
use sb_middleware::utils::db_utils::{get_entity_list_view, get_entity_view, IdentIdName, Pagination, QryOrder, ViewFieldSelector, with_not_found_err};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub from_user: Thing,
    pub to_user: Thing,
    pub post: Option<Thing>,
    pub content: String,
    pub offer_amount: u64,
    pub status: String,
    // #[serde(skip_serializing)]
    pub r_created: Option<String>,
    // #[serde(skip_serializing)]
    pub r_updated: Option<String>,

}

pub struct TaskRequestDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "task_request";
const TABLE_COL_POST: &str = sb_community::entity::post_entitiy::TABLE_NAME;
const TABLE_COL_USER: &str = sb_user_auth::entity::local_user_entity::TABLE_NAME;

impl<'a> TaskRequestDbService<'a> {

    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD post ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_POST}>;
    DEFINE INDEX post_idx ON TABLE {TABLE_NAME} COLUMNS post;
    DEFINE FIELD from_user ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE INDEX from_user_idx ON TABLE {TABLE_NAME} COLUMNS from_user;
    DEFINE FIELD to_user ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE INDEX to_user_idx ON TABLE {TABLE_NAME} COLUMNS to_user;
    DEFINE FIELD content ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD status ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD offer_amount ON TABLE {TABLE_NAME} TYPE number;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.db
            .query(sql)
            .await?;

        &mutation.check().expect("should mutate taskRequest");

        Ok(())
    }

    pub async fn create(&self, record: TaskRequest) -> CtxResult<TaskRequest> {
         let res = self.db
             .create(TABLE_NAME)
             .content(record)
             .await
             .map_err(CtxError::from(self.ctx))
             .map(|v: Option<TaskRequest>| v.unwrap());

        // let things: Vec<Domain> = self.db.select(TABLE_NAME).await.ok().unwrap();
        // dbg!(things);
        res
    }

    pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(&self, ident_id_name: &IdentIdName) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), ident_id_name).await?;
        with_not_found_err(opt, self.ctx, ident_id_name.to_string().as_str())
    }

}

