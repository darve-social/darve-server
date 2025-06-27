use crate::{
    database::client::Db,
    entities::user_notification::UserNotification,
    interfaces::repositories::user_notifications::{
        GetNotificationOptions, UserNotificationsInterface,
    },
    middleware::{error::AppError, utils::string_utils::get_str_thing},
};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use surrealdb::sql::Thing;

#[derive(Debug)]
pub struct UserNotificationsRepository {
    client: Arc<Db>,
}

impl UserNotificationsRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self { client }
    }

    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!(
            " 
        DEFINE TABLE IF NOT EXISTS notifications SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS event       ON TABLE notifications TYPE string;
        DEFINE FIELD IF NOT EXISTS title       ON TABLE notifications TYPE string;
        DEFINE FIELD IF NOT EXISTS created_by  ON TABLE notifications TYPE record<local_user>;
        DEFINE FIELD IF NOT EXISTS metadata    ON TABLE notifications FLEXIBLE TYPE option<object>;
        DEFINE FIELD IF NOT EXISTS created_at  ON TABLE notifications TYPE datetime DEFAULT time::now();

        DEFINE TABLE IF NOT EXISTS user_notifications TYPE RELATION IN local_user OUT notifications ENFORCED SCHEMAFULL PERMISSIONS NONE;
        DEFINE FIELD IF NOT EXISTS is_read    ON TABLE user_notifications TYPE bool DEFAULT false;
        DEFINE INDEX IF NOT EXISTS in_out_idx ON user_notifications FIELDS in, out;
    "
        );
        let mutation = self.client.query(sql).await?;

        mutation.check().expect("should mutate local_user");

        Ok(())
    }
}

#[async_trait]
impl UserNotificationsInterface for UserNotificationsRepository {
    async fn create(
        &self,
        creator: &str,
        title: &str,
        n_type: &str,
        receivers: &Vec<String>,
        metadata: Option<Value>,
    ) -> Result<UserNotification, AppError> {
        let receiver_things = receivers
            .iter()
            .map(|id| get_str_thing(&id))
            .collect::<Result<Vec<Thing>, AppError>>()?;

        let query = r#"
            BEGIN TRANSACTION;
            
            LET $notification = CREATE ONLY notifications CONTENT {
                event: $event,
                title: $title,
                created_by:$created_by,
                metadata: $metadata,
            };
            LET $n_id = $notification.id;

            FOR $user_id IN $receivers {
                RELATE $user_id -> user_notifications -> $n_id SET is_read = false;
            };
            
            COMMIT TRANSACTION;
            RETURN {
                id: record::id($notification.id),
                created_by: record::id($notification.created_by),
                event: $notification.event,
                title: $notification.title,
                metadata: $notification.metadata,
                created_at: $notification.created_at
            };

        "#;

        let mut res = self
            .client
            .query(query)
            .bind(("event", n_type.to_string()))
            .bind(("title", title.to_string()))
            .bind(("created_by", get_str_thing(creator)?))
            .bind(("metadata", metadata))
            .bind(("receivers", receiver_things))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;

        let data = res
            .take::<Option<UserNotification>>(res.num_statements() - 1)
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?
            .unwrap();

        Ok(data)
    }

    async fn get_by_user(
        &self,
        user_id: &str,
        options: GetNotificationOptions,
    ) -> Result<Vec<UserNotification>, AppError> {
        let is_read_query = if options.is_read.is_some() {
            "AND is_read = $is_read"
        } else {
            ""
        };
        let query = format!(
            " SELECT out.*,
                    record::id(out.id) AS out.id,
                    record::id(out.created_by) AS out.created_by, 
                    is_read as out.is_read, 
                    out.created_at as created_at 
                FROM user_notifications
                WHERE in = $user_id {}
                ORDER BY created_at {}
                LIMIT $limit START $start;",
            is_read_query, options.order_dir
        );
        let mut res = self
            .client
            .query(&query)
            .bind(("user_id", get_str_thing(user_id)?))
            .bind(("start", options.start))
            .bind(("limit", options.limit))
            .bind(("is_read", options.is_read))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;
        let data = res.take::<Vec<UserNotification>>((0, "out"))?;
        Ok(data)
    }

    async fn read(&self, id: &str, user_id: &str) -> Result<(), AppError> {
        let _ = self
            .client
            .query("UPDATE user_notifications SET is_read=$is_read WHERE out=$id AND in=$user_id")
            .bind(("id", Thing::from(("notifications", id))))
            .bind(("user_id", get_str_thing(user_id)?))
            .bind(("is_read", true))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;
        Ok(())
    }
    async fn read_all(&self, user_id: &str) -> Result<(), AppError> {
        let _ = self
            .client
            .query("UPDATE user_notifications SET is_read=$is_read WHERE in=$user_id")
            .bind(("user_id", get_str_thing(user_id)?))
            .bind(("is_read", true))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;
        Ok(())
    }
    async fn get_count(&self, user_id: &str, is_read: Option<bool>) -> Result<u64, AppError> {
        let is_read_query = if is_read.is_some() {
            "AND is_read = $is_read"
        } else {
            ""
        };
        let query = format!(
            "SELECT count() FROM user_notifications WHERE in = $user_id {} GROUP ALL;",
            is_read_query,
        );
        let mut res = self
            .client
            .query(&query)
            .bind(("user_id", get_str_thing(user_id)?))
            .bind(("is_read", is_read))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;
        let data = res.take::<Option<u64>>((0, "count"))?;
        Ok(data.unwrap_or(0))
    }
}
