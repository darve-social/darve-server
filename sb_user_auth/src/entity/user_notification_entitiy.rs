use std::fmt::Display;

use serde::{Deserialize, Serialize};
use strum::Display;
use surrealdb::sql::{Id, Thing};

use sb_middleware::utils::db_utils::{get_entity_list, IdentIdName, Pagination, QryOrder};
use sb_middleware::db;
use sb_middleware::{
    ctx::Ctx,
    error::{CtxError, CtxResult, AppError},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserNotification {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub user: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_ident: Option<String>,
    pub event: UserNotificationEvent,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
}

impl From<UserNotification> for axum::response::sse::Event {
    fn from(value: UserNotification) -> Self {
        Self::default().data(value.event.to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Display)]
pub enum UserNotificationEvent {
    UserFollowAdded { username: String },
    UserTaskRequestComplete { task_id: Thing, delivered_by: Thing, requested_by: Thing, deliverables: Vec<String> },
}

pub struct UserNotificationDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "user_notification";
const USER_TABLE: &str = crate::entity::local_user_entity::TABLE_NAME;

impl<'a> UserNotificationDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD user ON TABLE {TABLE_NAME} TYPE record<{USER_TABLE}>;
    DEFINE INDEX user_idx ON TABLE {TABLE_NAME} COLUMNS user;
    DEFINE FIELD event ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD content ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD event_ident ON TABLE {TABLE_NAME} TYPE option<string>;
    // will use ulid to sort by time DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE INDEX r_created_idx ON TABLE {TABLE_NAME} COLUMNS r_created;

");

        let mutation = self.db
            .query(sql)
            .await?;
        &mutation.check().expect("should mutate user_notification");

        Ok(())
    }
    /*pub async fn get_by_user(&self, user_id: Thing, from: i32, count: i8) -> CtxResult<Vec<UserNotification>> {
        get_entity_list::<UserNotification>(self.db, TABLE_NAME.to_string(), &IdentIdName::ColumnIdent { column: TABLE_COL_USER.to_string(), val: user_id.to_raw(), rec: true},
                                        Some(Pagination { order_by: Option::from("r_created".to_string()),order_dir:Some(QryOrder::DESC), count: 20, start: 0 }
                                        )).await
    }*/

    pub async fn create(&self, mut record: UserNotification) -> CtxResult<UserNotification> {
        record.id = Some(Thing::from((TABLE_NAME, Id::ulid())));
        let userNotification = self.db
            .create(TABLE_NAME)
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<UserNotification>| v.unwrap())?;
        Ok(userNotification)
    }
}

