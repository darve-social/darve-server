use super::super::table_names::{ACCESS_TABLE_NAME, DISC_USER_TABLE_NAME, POST_USER_TABLE_NAME};
use crate::database::client::Db;
use crate::entities::community::discussion_entity::TABLE_NAME as DISC_TABLE_NAME;
use crate::entities::community::post_entity::{
    PostType, PostUserStatus, TABLE_NAME as POST_TABLE_NAME,
};
use crate::entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME;
use crate::interfaces::repositories::discussion_user::DiscussionUserRepositoryInterface;
use crate::middleware::error::{AppError, AppResult};
use crate::middleware::utils::db_utils::{Pagination, QryOrder, ViewFieldSelector};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use surrealdb::sql::Thing;

#[derive(Debug)]
pub struct DiscussionUserRepository {
    client: Arc<Db>,
}

impl DiscussionUserRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self { client }
    }
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
        DEFINE TABLE IF NOT EXISTS {DISC_USER_TABLE_NAME} TYPE RELATION IN {DISC_TABLE_NAME} OUT {USER_TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
        DEFINE INDEX IF NOT EXISTS in_out_unique_idx ON {DISC_USER_TABLE_NAME} FIELDS in, out UNIQUE;
        DEFINE FIELD IF NOT EXISTS nr_unread ON TABLE {DISC_USER_TABLE_NAME} TYPE int VALUE math::max([$value, 0]) DEFAULT 0;
        DEFINE FIELD IF NOT EXISTS created_at ON TABLE {DISC_USER_TABLE_NAME} TYPE datetime DEFAULT time::now();
        DEFINE FIELD IF NOT EXISTS updated_at ON TABLE {DISC_USER_TABLE_NAME} TYPE datetime DEFAULT time::now();
        DEFINE FIELD IF NOT EXISTS latest_post ON TABLE {DISC_USER_TABLE_NAME} TYPE option<record<{POST_TABLE_NAME}>>;
");
        let mutation = self.client.query(sql).await?;

        mutation
            .check()
            .expect("should mutate DiscussionUserRepository");

        Ok(())
    }
}

#[async_trait]
impl DiscussionUserRepositoryInterface for DiscussionUserRepository {
    async fn create(&self, disc_id: &str, user_ids: Vec<Thing>) -> AppResult<()> {
        let _ = self
            .client
            .query(format!("LET $latest_post = (SELECT id FROM {POST_TABLE_NAME} WHERE belongs_to=$disc AND type=$public_type ORDER BY id DESC LIMIT 1)[0].id"))
            .query(format!("RELATE $disc->{DISC_USER_TABLE_NAME}->$users SET latest_post=$latest_post, nr_unread=0"))
            .bind(("disc", Thing::from((DISC_TABLE_NAME, disc_id))))
            .bind(("users", user_ids))
              .bind(("public_type", PostType::Public))
            .await?;

        Ok(())
    }

    async fn set_new_latest_post(
        &self,
        disc_id: &str,
        user_ids: Vec<&String>,
        latest_post: &str,
        increase_unread_for_user_ids: Vec<&String>,
    ) -> AppResult<()> {
        let users = user_ids
            .into_iter()
            .map(|id| Thing::from((USER_TABLE_NAME, id.as_str())))
            .collect::<Vec<Thing>>();

        let increase_users = increase_unread_for_user_ids
            .into_iter()
            .map(|id| Thing::from((USER_TABLE_NAME, id.as_str())))
            .collect::<Vec<Thing>>();

        let _ = self
            .client
            .query(format!(
                "UPDATE $disc->{DISC_USER_TABLE_NAME}
                    SET latest_post=$post,
                        nr_unread+= (IF out IN $increase_for_users THEN 1 ELSE 0 END),
                        updated_at=time::now()
                    WHERE out IN $users;"
            ))
            .bind(("disc", Thing::from((DISC_TABLE_NAME, disc_id))))
            .bind(("increase_for_users", increase_users))
            .bind(("users", users))
            .bind(("post", Thing::from((POST_TABLE_NAME, latest_post))))
            .await?;

        Ok(())
    }

    async fn update_latest_post(&self, disc_id: &str, user_ids: Vec<String>) -> AppResult<()> {
        let users = user_ids
            .into_iter()
            .map(|id| Thing::from((USER_TABLE_NAME, id.as_str())))
            .collect::<Vec<Thing>>();
        let _ = self
            .client
            .query(format!("UPDATE $disc->{DISC_USER_TABLE_NAME}
                    SET nr_unread-= (IF latest_post->{POST_USER_TABLE_NAME}[WHERE out=$parent.out AND status=$read_status] THEN 0 ELSE 1 END),
                        latest_post=(SELECT id FROM {POST_TABLE_NAME} WHERE belongs_to=$disc AND (type=$public_type OR $parent.out IN <-{ACCESS_TABLE_NAME}.in) ORDER BY id DESC LIMIT 1)[0].id,
                        updated_at=time::now()
                    WHERE out IN $users;"))
            .bind(("disc", Thing::from((DISC_TABLE_NAME, disc_id))))
            .bind(("users", users))
            .bind(("public_type", PostType::Public))
            .bind(("read_status", PostUserStatus::Seen))
            .await?;
        Ok(())
    }

    async fn decrease_unread_count(&self, disc_id: &str, user_ids: Vec<String>) -> AppResult<()> {
        let users = user_ids
            .into_iter()
            .map(|id| Thing::from((USER_TABLE_NAME, id.as_str())))
            .collect::<Vec<Thing>>();
        let _ = self
            .client
            .query(format!(
                "UPDATE $disc->{DISC_USER_TABLE_NAME} SET nr_unread-=1 WHERE out IN $users;"
            ))
            .bind(("disc", Thing::from((DISC_TABLE_NAME, disc_id))))
            .bind(("users", users))
            .await?;

        Ok(())
    }

    async fn get_by_user<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        user_id: &str,
        pad: Pagination,
        require_latest_post: bool,
    ) -> AppResult<Vec<T>> {
        let fields = T::get_select_query_fields();
        let order_dir = pad.order_dir.unwrap_or(QryOrder::DESC);
        let latest_post_cond = if require_latest_post {
            "AND latest_post != NONE"
        } else {
            ""
        };
        let mut res = self
            .client
            .query(format!(
                "SELECT {fields}, updated_at FROM {DISC_USER_TABLE_NAME} 
                WHERE out=$user {latest_post_cond} ORDER BY updated_at {order_dir} 
                LIMIT $limit START $start;"
            ))
            .bind(("limit", pad.count))
            .bind(("start", pad.start))
            .bind(("user", Thing::from((USER_TABLE_NAME, user_id))))
            .await?;

        let data = res.take::<Vec<T>>(0)?;
        Ok(data)
    }

    async fn remove(&self, disc_id: &str, user_ids: Vec<Thing>) -> AppResult<()> {
        let _ = self
            .client
            .query(format!(
                "DELETE $disc->{DISC_USER_TABLE_NAME} WHERE out IN $users;"
            ))
            .bind(("disc", Thing::from((DISC_TABLE_NAME, disc_id))))
            .bind(("users", user_ids))
            .await?
            .check()?;
        Ok(())
    }
}
