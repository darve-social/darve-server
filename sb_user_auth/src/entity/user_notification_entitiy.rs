use std::fmt::Display;

use serde::{Deserialize, Serialize};
use strum::Display;
use surrealdb::sql::{Id, Thing};

use crate::entity::follow_entitiy::FollowDbService;
use sb_middleware::db;
use sb_middleware::error::AppResult;
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserNotification {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub user: Thing,
    pub event: UserNotificationEvent,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Display)]
pub enum UserNotificationEvent {
    UserFollowAdded { username: String },
    UserTaskRequestComplete { task_id: Thing, delivered_by: Thing, requested_by: Thing, deliverables: Vec<String> },
    UserTaskRequestCreated { task_id: Thing, from_user: Thing, to_user: Thing },
    UserTaskRequestReceived { task_id: Thing, from_user: Thing, to_user: Thing },
}

pub struct UserNotificationDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

impl<'a> UserNotificationDbService<'a> {
    pub async fn notify_users(&self, user_ids: Vec<Thing>, event: &UserNotificationEvent, content: &str) -> AppResult<()> {
        let qry: Vec<String> = user_ids.into_iter().map(|u| {
            self.create_qry(&UserNotification {
                id: None,
                user: u,
                event: event.clone(),
                content: content.to_string(),
                r_created: None,
            })
                .ok()
        })
            .filter(|v| v.is_some())
            .map(|v| v.unwrap())
            .collect();
        let res = self.db.query(qry.join("")).await?;
        res.check()?;
        Ok(())
    }

    pub async fn notify_user_followers(&self, user_id: Thing, event: &UserNotificationEvent, content: &str) -> AppResult<()> {
        let notify_followers_task_given_qry: Vec<Thing> = FollowDbService { db: self.db, ctx: self.ctx }.user_follower_ids(user_id.clone()).await?;
        self.notify_users(notify_followers_task_given_qry, event, content).await
    }

    pub fn create_qry(&self, u_notification: &UserNotification) -> AppResult<String> {
        let event_json = serde_json::to_string(&u_notification.event)?;
        Ok(format!("INSERT INTO {TABLE_NAME} {{user: {}, event:\"{event_json}\", content:\"\" }};", u_notification.user))
    }
}

pub const TABLE_NAME: &str = "user_notification";
const USER_TABLE: &str = crate::entity::local_user_entity::TABLE_NAME;

impl<'a> UserNotificationDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD user ON TABLE {TABLE_NAME} TYPE record<{USER_TABLE}>;
    DEFINE INDEX user_idx ON TABLE {TABLE_NAME} COLUMNS user;
    DEFINE FIELD event ON TABLE {TABLE_NAME} TYPE {{UserFollowAdded:{{username: string}}}} | {{UserTaskRequestComplete:{{task_id: record, delivered_by: record<{USER_TABLE}>, requested_by: record<{USER_TABLE}>, deliverables: set<string>}}}};
    DEFINE FIELD content ON TABLE {TABLE_NAME} TYPE string;
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

