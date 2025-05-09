use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::routes::community_routes::DiscussionNotificationEvent;
use sb_middleware::db;
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscussionNotification {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub event_ident: Option<String>,
    pub event: DiscussionNotificationEvent,
    pub content: String,
    pub r_created: Option<String>,
}

/*impl From<DiscussionNotification> for axum::response::sse::Event {
    fn from(value: DiscussionNotification) -> Self {
        Self::default().data(value.event)
    }
}*/

pub struct DiscussionNotificationDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "notification";
const POST_TABLE: &str = crate::entity::post_entitiy::TABLE_NAME;
const DISCUSSION_TABLE: &str = crate::entity::discussion_entitiy::TABLE_NAME;
const TOPIC_TABLE: &str = crate::entity::discussion_topic_entitiy::TABLE_NAME;

impl<'a> DiscussionNotificationDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS event ON TABLE {TABLE_NAME} TYPE {{DiscussionPostAdded: {{ discussion_id: record<{DISCUSSION_TABLE}>, topic_id: option<record<{TOPIC_TABLE}>>, post_id: record<{POST_TABLE}> }}}}
        | {{DiscussionPostReplyNrIncreased: {{ discussion_id: record<{DISCUSSION_TABLE}>, topic_id: option<record<{TOPIC_TABLE}>>, post_id: record<{POST_TABLE}> }}}}
        | {{DiscussionPostReplyAdded: {{ discussion_id: record<{DISCUSSION_TABLE}>, topic_id: option<record<{TOPIC_TABLE}>>, post_id: record<{POST_TABLE}> }}}};

    DEFINE FIELD IF NOT EXISTS content ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD IF NOT EXISTS event_ident ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();

");

        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate domain");

        Ok(())
    }
    /*pub async fn get_by_user(&self, user_id: Thing, from: i32, count: i8) -> CtxResult<Vec<Notification>> {
        get_entity_list::<Notification>(self.db, TABLE_NAME.to_string(), &IdentIdName::ColumnIdent { column: TABLE_COL_USER.to_string(), val: user_id.to_raw(), rec: true},
                                        Some(Pagination { order_by: Option::from("r_created".to_string()),order_dir:Some(QryOrder::DESC), count: 20, start: 0 }
                                        )).await
    }*/

    pub async fn create(
        &self,
        record: DiscussionNotification,
    ) -> CtxResult<DiscussionNotification> {
        let notification = self
            .db
            .create(TABLE_NAME)
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<DiscussionNotification>| v.unwrap())?;
        Ok(notification)
    }
}
