use std::fmt::Display;

use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use sb_middleware::utils::db_utils::{get_entity_list, IdentIdName, Pagination, QryOrder};
use sb_middleware::db;
use sb_middleware::{
    ctx::Ctx,
    error::{CtxError, CtxResult, AppError},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notification {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_ident: Option<String>,
    pub event: String,
    pub content: String,
    pub r_created: Option<String>,
}

impl From<Notification> for axum::response::sse::Event {
    fn from(value: Notification) -> Self {
        Self::default().data(value.event)
    }
}

pub struct NotificationDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "notification";
const TABLE_COL_USER: &str = crate::entity::local_user_entity::TABLE_NAME;

impl<'a> NotificationDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD event ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD content ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD event_ident ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();

");

        let mutation = self.db
            .query(sql)
            .await?;
        &mutation.check().expect("should mutate domain");

        Ok(())
    }
    pub async fn get_by_user(&self, user_id: Thing, from: i32, count: i8) -> CtxResult<Vec<Notification>> {
        get_entity_list::<Notification>(self.db, TABLE_NAME.to_string(), &IdentIdName::ColumnIdent { column: TABLE_COL_USER.to_string(), val: user_id.to_raw(), rec: true},
                                        Some(Pagination { order_by: Option::from("r_created".to_string()),order_dir:Some(QryOrder::DESC), count: 20, start: 0 }
                                        )).await
    }

    pub async fn create(&self, record: Notification) -> CtxResult<Notification> {
        let notification = self.db
            .create(TABLE_NAME)
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<Notification>| v.unwrap())?;
        Ok(notification)
    }
}

