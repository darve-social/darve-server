use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum::Display;
use surrealdb::sql::{to_value, Id, Thing, Value};

use crate::entity::follow_entitiy::FollowDbService;
use sb_middleware::db;
use sb_middleware::error::AppResult;
use sb_middleware::utils::db_utils::QryBindingsVal;
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
#[serde(tag = "type")]
pub enum UserNotificationEvent {
    // !!! NOTE when changing enum also change db DEFINE FIELD event type
    UserFollowAdded {
        username: String,
        follows_username: String,
    },
    UserTaskRequestCreated {
        task_id: Thing,
        from_user: Thing,
        to_user: Thing,
    },
    UserTaskRequestReceived {
        task_id: Thing,
        from_user: Thing,
        to_user: Thing,
    },
    UserTaskRequestDelivered {
        task_id: Thing,
        deliverable: Thing,
        delivered_by: Thing,
    },
    UserChatMessage,
    UserCommunityPost,
}

pub struct UserNotificationDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

impl<'a> UserNotificationDbService<'a> {

    pub async fn notify_users(
        &self,
        user_ids: Vec<Thing>,
        event: &UserNotificationEvent,
        content: &str,
    ) -> AppResult<()> {
        let qry: Vec<QryBindingsVal<Value>> = user_ids
            .into_iter()
            .enumerate()
            .map(|i_uid| {
                self.create_qry(
                    &UserNotification {
                        id: None,
                        user: i_uid.1,
                        event: event.clone(),
                        content: content.to_string(),
                        r_created: None,
                    },
                    i_uid.0,
                )
                .ok()
            })
            .filter(|v| v.is_some())
            .map(|v| v.unwrap())
            .collect();
        let qrys_bindings =
            qry.into_iter()
                .fold((vec![], HashMap::new()), |mut qrys_bindings, qbv| {
                    qrys_bindings.0.push(qbv.get_query_string());
                    qrys_bindings.1.extend(qbv.get_bindings());
                    qrys_bindings
                });
        let qry = self.db.query(qrys_bindings.0.join(""));
        let qry = qrys_bindings
            .1
            .into_iter()
            .fold(qry, |qry, n_val| qry.bind(n_val));
        let res = qry.await?;
        res.check()?;
        Ok(())
    }

    pub async fn notify_user_followers(
        &self,
        user_id: Thing,
        event: &UserNotificationEvent,
        content: &str,
    ) -> AppResult<()> {
        let notify_followers_task_given_qry: Vec<Thing> = FollowDbService {
            db: self.db,
            ctx: self.ctx,
        }
        .user_follower_ids(user_id.clone())
        .await?;
        self.notify_users(notify_followers_task_given_qry, event, content)
            .await
    }

    pub fn create_qry(
        &self,
        u_notification: &UserNotification,
        qry_ident: usize,
    ) -> AppResult<QryBindingsVal<Value>> {
        let mut bindings: HashMap<String, Value> = HashMap::new();
        let event_val = to_value(u_notification.event.clone()).map_err(|e| AppError::SurrealDb {source: e.to_string()})?;
        bindings.insert(format!("event_json_{qry_ident}"), event_val);
        bindings.insert(format!("content_{qry_ident}"), to_value(String::new()).map_err(|e| AppError::SurrealDb {source: e.to_string()})?);
        bindings.insert(format!("user_id_{qry_ident}"), to_value(u_notification.user.clone()).map_err(|e| AppError::SurrealDb {source: e.to_string()})?);
        let qry = format!("INSERT INTO {TABLE_NAME} {{user: (type::record($user_id_{qry_ident})), event:($event_json_{qry_ident}), content:($content_{qry_ident}) }};");
        Ok(QryBindingsVal::new(qry, bindings))
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
    DEFINE FIELD event ON TABLE {TABLE_NAME} TYPE
     {{ type: \"UserFollowAdded\", username: string, follows_username: string}}
     | {{ type: \"UserTaskRequestDelivered\", task_id: record, delivered_by: record<{USER_TABLE}>, deliverable: record}}
     | {{ type: \"UserTaskRequestCreated\", task_id: record, from_user: record<{USER_TABLE}>, to_user: record<{USER_TABLE}>}}
     | {{ type: \"UserTaskRequestReceived\", task_id: record, from_user: record<{USER_TABLE}>, to_user: record<{USER_TABLE}>}}
     | {{ type: \"UserChatMessage\"}}
     | {{ type: \"UserCommunityPost\"}};
    DEFINE FIELD content ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    // will use ulid to sort by time DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    // DEFINE INDEX r_created_idx ON TABLE {TABLE_NAME} COLUMNS r_created;

");

        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate user_notification");

        Ok(())
    }

    pub async fn create(&self, mut record: UserNotification) -> CtxResult<UserNotification> {
        record.id = Some(Thing::from((TABLE_NAME, Id::ulid())));
        let user_notification = self
            .db
            .create(TABLE_NAME)
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<UserNotification>| v.unwrap())?;
        Ok(user_notification)
    }
}
