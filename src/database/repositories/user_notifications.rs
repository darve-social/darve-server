use crate::{
    database::client::Db,
    entities::user_notification::UserNotification,
    interfaces::repositories::user_notifications::UserNotificationsInterface,
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
        DEFINE FIELD IF NOT EXISTS type        ON TABLE notifications TYPE string;
        DEFINE FIELD IF NOT EXISTS title       ON TABLE notifications TYPE string;
        DEFINE FIELD IF NOT EXISTS created_by  ON TABLE notifications TYPE record<local_user>;
        DEFINE FIELD IF NOT EXISTS content     ON TABLE notifications TYPE option<string>;
        DEFINE FIELD IF NOT EXISTS metadata    ON TABLE notifications TYPE option<object>;

        DEFINE TABLE IF NOT EXISTS user_notifications TYPE RELATION IN local_user OUT notifications ENFORCED SCHEMAFULL PERMISSIONS NONE;
        DEFINE FIELD IF NOT EXISTS created_at ON TABLE user_notifications TYPE datetime DEFAULT time::now();
        DEFINE FIELD IF NOT EXISTS is_read    ON TABLE user_notifications TYPE bool DEFAULT false;
        DEFINE INDEX IF NOT EXISTS in_out_idx ON user_notifications FIELDS in, out;

    "
        );
        let local_user_mutation = self.client.query(sql).await?;

        local_user_mutation
            .check()
            .expect("should mutate local_user");

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
        content: Option<String>,
        metadata: Option<Value>,
    ) -> Result<UserNotification, AppError> {
        let receiver_things = receivers
            .iter()
            .map(|id| get_str_thing(&id))
            .collect::<Result<Vec<Thing>, AppError>>()?;

        let query = r#"
            BEGIN TRANSACTION;
            
            LET $notification = CREATE ONLY notifications CONTENT {
                type: $n_type,
                title: $title,
                created_by:$created_by,
                content: $content,
                metadata:$metadata,
            };
            LET $n_id = $notification.id;

            FOR $user_id IN $receivers {
                RELATE $user_id -> user_notifications -> $n_id SET is_read = false;
            };
            
            COMMIT TRANSACTION;
            RETURN $notification;
        "#;

        let mut res = self
            .client
            .query(query)
            .bind(("n_type", n_type.to_string()))
            .bind(("title", title.to_string()))
            .bind(("created_by", get_str_thing(creator)?))
            .bind(("content", content))
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

    async fn get_by_user(&self, user_id: &str) -> Result<Vec<UserNotification>, AppError> {
        let user_thing = get_str_thing(user_id)?;
        let mut res = self
            .client
            .query("SELECT ->out.* AS *, is_read FROM user_notifications WHERE in = $user_id")
            .bind(("user_id", user_thing))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;
        let data: Vec<UserNotification> = res.take(0).unwrap_or_default();
        Ok(data)
    }

    async fn get_by_id(&self, id: &str, user_id: &str) -> Result<UserNotification, AppError> {
        let notification_thing = get_str_thing(id)?;
        let user_thing = get_str_thing(user_id)?;
        let mut res = self
            .client
            .query("SELECT ->out.* AS *, is_read FROM user_notifications WHERE in = $user AND out = $notification")
            .bind(("notification", notification_thing))
            .bind(("user", user_thing))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;
        let data: Option<UserNotification> = res.take(0).unwrap();
        data.ok_or_else(|| AppError::EntityFailIdNotFound {
            ident: id.to_string(),
        })
    }

    async fn update(&self, id: &str, is_read: bool) -> Result<(), AppError> {
        let notification_thing = get_str_thing(id)?;

        self.client
            .query("UPDATE $id SET read = $is_read")
            .bind(("id", notification_thing))
            .bind(("is_read", is_read))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;

        Ok(())
    }
}
