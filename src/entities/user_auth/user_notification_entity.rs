// use serde::{Deserialize, Serialize};
// use std::collections::HashMap;
// use strum::Display;
// use surrealdb::sql::{to_value, Id, Thing, Value};

// use crate::database::client::Db;
// use middleware::error::AppResult;
// use middleware::utils::db_utils::{
//     get_entity_list_view, IdentIdName, Pagination, QryBindingsVal, ViewFieldSelector,
// };
// use middleware::utils::extractor_utils::DiscussionParams;
// use middleware::{
//     ctx::Ctx,
//     error::{AppError, CtxError, CtxResult},
// };

// use crate::entities::community::post_entity;
// use crate::middleware;

// use super::local_user_entity;

// #[derive(Clone, Debug, Serialize, Deserialize)]
// pub struct UserNotification {
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub id: Option<Thing>,
//     pub user: Thing,
//     pub event: UserNotificationEvent,
//     pub content: String,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub r_created: Option<String>,
// }

// #[derive(Serialize, Deserialize, Debug, Clone, Display)]
// #[serde(tag = "type", content = "value")]
// pub enum UserNotificationEvent {
//     // !!! NOTE when changing enum also change db DEFINE FIELD IF NOT EXISTS event type
//     UserFollowAdded {
//         username: String,
//         follows_username: String,
//     },
//     UserTaskRequestCreated {
//         task_id: Thing,
//         from_user: Thing,
//         to_user: Thing,
//     },
//     UserTaskRequestReceived {
//         task_id: Thing,
//         from_user: Thing,
//         to_user: Thing,
//     },
//     UserTaskRequestDelivered {
//         task_id: Thing,
//         deliverable: Thing,
//         delivered_by: Thing,
//     },
//     UserChatMessage,
//     UserCommunityPost,
//     UserBalanceUpdate,
//     UserLikePost {
//         user_id: Thing,
//         post_id: Thing,
//     },
// }

// pub struct UserNotificationDbService<'a> {
//     pub db: &'a Db,
//     pub ctx: &'a Ctx,
// }

// impl<'a> UserNotificationDbService<'a> {
//     pub async fn get_by_user_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
//         &self,
//         user_id: Thing,
//         params: DiscussionParams,
//     ) -> CtxResult<Vec<T>> {
//         let pagination = Some(Pagination {
//             order_by: None, // uses ulid  so not needed
//             order_dir: None,
//             count: params.count.unwrap_or(20),
//             start: params.start.unwrap_or(0),
//         });
//         get_entity_list_view::<T>(
//             self.db,
//             TABLE_NAME.to_string(),
//             &IdentIdName::ColumnIdent {
//                 column: USER_FIELD_NAME.to_string(),
//                 rec: true,
//                 val: user_id.to_raw(),
//             },
//             pagination,
//         )
//         .await
//     }

//     pub async fn notify_users(
//         &self,
//         user_ids: Vec<Thing>,
//         event: &UserNotificationEvent,
//         content: &str,
//     ) -> AppResult<()> {
//         let qry: Vec<QryBindingsVal<Value>> = user_ids
//             .into_iter()
//             .enumerate()
//             .map(|i_uid| {
//                 self.create_qry(
//                     &UserNotification {
//                         id: None,
//                         user: i_uid.1,
//                         event: event.clone(),
//                         content: content.to_string(),
//                         r_created: None,
//                     },
//                     i_uid.0,
//                 )
//                 .ok()
//             })
//             .filter(|v| v.is_some())
//             .map(|v| v.unwrap())
//             .collect();
//         let qrys_bindings =
//             qry.into_iter()
//                 .fold((vec![], HashMap::new()), |mut qrys_bindings, qbv| {
//                     qrys_bindings.0.push(qbv.get_query_string());
//                     qrys_bindings.1.extend(qbv.get_bindings());
//                     qrys_bindings
//                 });
//         let qry = self.db.query(qrys_bindings.0.join(""));
//         let qry = qrys_bindings
//             .1
//             .into_iter()
//             .fold(qry, |qry, n_val| qry.bind(n_val));

//         let res = qry.await?;
//         res.check()?;
//         Ok(())
//     }

//     pub fn create_qry(
//         &self,
//         u_notification: &UserNotification,
//         qry_ident: usize,
//     ) -> AppResult<QryBindingsVal<Value>> {
//         let mut bindings: HashMap<String, Value> = HashMap::new();
//         let event_val =
//             to_value(u_notification.event.clone()).map_err(|e| AppError::SurrealDb {
//                 source: e.to_string(),
//             })?;
//         bindings.insert(format!("event_json_{qry_ident}"), event_val);
//         bindings.insert(
//             format!("content_{qry_ident}"),
//             to_value(String::new()).map_err(|e| AppError::SurrealDb {
//                 source: e.to_string(),
//             })?,
//         );
//         bindings.insert(
//             format!("user_id_{qry_ident}"),
//             to_value(u_notification.user.clone()).map_err(|e| AppError::SurrealDb {
//                 source: e.to_string(),
//             })?,
//         );
//         let qry = format!("INSERT INTO {TABLE_NAME} {{ {USER_FIELD_NAME}: (type::record($user_id_{qry_ident})), event:($event_json_{qry_ident}), content:($content_{qry_ident}) }};");
//         Ok(QryBindingsVal::new(qry, bindings))
//     }
// }

// pub const TABLE_NAME: &str = "user_notification";
// const USER_TABLE: &str = local_user_entity::TABLE_NAME;
// const POST_TABLE: &str = post_entity::TABLE_NAME;
// const USER_FIELD_NAME: &str = "user";

// impl<'a> UserNotificationDbService<'a> {
//     pub async fn mutate_db(&self) -> Result<(), AppError> {
//         let sql = format!("
//     DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
//     DEFINE FIELD IF NOT EXISTS {USER_FIELD_NAME} ON TABLE {TABLE_NAME} TYPE record<{USER_TABLE}>;
//     DEFINE INDEX IF NOT EXISTS {USER_FIELD_NAME}_idx ON TABLE {TABLE_NAME} COLUMNS {USER_FIELD_NAME};
//     DEFINE FIELD IF NOT EXISTS event ON TABLE {TABLE_NAME} TYPE
//      {{ type: \"UserFollowAdded\", value: {{username: string, follows_username: string}} }}
//      | {{ type: \"UserTaskRequestDelivered\", value:{{ task_id: record, delivered_by: record<{USER_TABLE}>, deliverable: record}} }}
//      | {{ type: \"UserTaskRequestCreated\", value:{{ task_id: record, from_user: record<{USER_TABLE}>, to_user: record<{USER_TABLE}>}} }}
//      | {{ type: \"UserTaskRequestReceived\", value:{{ task_id: record, from_user: record<{USER_TABLE}>, to_user: record<{USER_TABLE}>}} }}
//      | {{ type: \"UserChatMessage\"}}
//      | {{ type: \"UserBalanceUpdate\"}}
//      | {{ type: \"UserCommunityPost\"}}
//      | {{ type: \"UserLikePost\", value:{{ post_id: record<{POST_TABLE}>, user_id: record<{USER_TABLE}>}} }};
//     DEFINE FIELD IF NOT EXISTS content ON TABLE {TABLE_NAME} TYPE string;
//     DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
//     // will use ulid to sort by time DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
//     // DEFINE INDEX IF NOT EXISTS r_created_idx ON TABLE {TABLE_NAME} COLUMNS r_created;

// ");

//         let mutation = self.db.query(sql).await?;
//         mutation.check().expect("should mutate user_notification");

//         Ok(())
//     }

//     pub async fn create(&self, mut record: UserNotification) -> CtxResult<UserNotification> {
//         record.id = Some(Thing::from((TABLE_NAME, Id::ulid())));
//         let user_notification = self
//             .db
//             .create(TABLE_NAME)
//             .content(record)
//             .await
//             .map_err(CtxError::from(self.ctx))
//             .map(|v: Option<UserNotification>| v.unwrap())?;
//         Ok(user_notification)
//     }
// }
