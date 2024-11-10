use std::fmt::Display;

use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use sb_middleware::db;
use sb_middleware::utils::db_utils::{get_entity_list, IdentIdName, Pagination, QryOrder};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Follow {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub r#in: Thing,
    pub out: Thing,
    pub r_created: Option<String>,
}


pub struct FollowDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "follow";
const TABLE_USER: &str = crate::entity::local_user_entity::TABLE_NAME;

impl<'a> FollowDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} TYPE RELATION FROM {TABLE_USER} TO {TABLE_USER} SCHEMAFULL PERMISSIONS none;
    DEFINE INDEX follow_idx ON TABLE {TABLE_NAME} COLUMNS in, out UNIQUE;
    // DEFINE FIELD in ON TABLE {TABLE_NAME} TYPE record<{TABLE_USER}>;
    // DEFINE FIELD out ON TABLE {TABLE_NAME} TYPE record<{TABLE_USER}>;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
");

        let mutation = self.db
            .query(sql)
            .await?;
        &mutation.check().expect("should mutate domain");

        Ok(())
    }
    /*pub async fn get_by_user(&self, user_id: Thing, from: i32, count: i8) -> CtxResult<Vec<Follow>> {
        get_entity_list::<Follow>(self.db, TABLE_NAME.to_string(), &IdentIdName::ColumnIdent { column: TABLE_COL_USER.to_string(), val: user_id.to_raw(), rec: true},
                                        Some(Pagination { order_by: Option::from("r_created".to_string()),order_dir:Some(QryOrder::DESC), count: 20, start: 0 }
                                        )).await
    }*/

    pub async fn create(&self, record: Follow) -> CtxResult<Follow> {
        let follow = self.db
            .create(TABLE_NAME)
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<Follow>| v.unwrap())?;
        Ok(follow)
    }
}

