use super::super::table_names::POST_USER_TABLE_NAME;
use crate::database::client::Db;
use crate::entities::community::post_entity::{PostUserStatus, TABLE_NAME as POST_TABLE_NAME};
use crate::entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME;
use crate::interfaces::repositories::post_user::PostUserRepositoryInterface;
use crate::middleware::error::{AppError, AppResult};
use async_trait::async_trait;
use std::sync::Arc;
use surrealdb::sql::Thing;
#[derive(Debug)]
pub struct PostUserRepository {
    client: Arc<Db>,
}

impl PostUserRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self { client }
    }
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
        DEFINE TABLE IF NOT EXISTS {POST_USER_TABLE_NAME} TYPE RELATION IN {POST_TABLE_NAME} OUT {USER_TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
        DEFINE INDEX IF NOT EXISTS in_out_unique_idx ON like FIELDS in, out UNIQUE;
        DEFINE FIELD IF NOT EXISTS created_at ON TABLE {POST_USER_TABLE_NAME} TYPE datetime DEFAULT time::now();
        DEFINE FIELD IF NOT EXISTS status ON TABLE {POST_USER_TABLE_NAME} TYPE int;
");
        let mutation = self.client.query(sql).await?;

        mutation.check().expect("should mutate PostUserRepository");

        Ok(())
    }
}

#[async_trait]
impl PostUserRepositoryInterface for PostUserRepository {
    async fn update(&self, user: Thing, post: Thing, status: u8) -> AppResult<()> {
        let _ = self
            .client
            .query(format!(
                "UPDATE $user<-{POST_USER_TABLE_NAME} SET status=$status WHERE in=$post"
            ))
            .bind(("post", post))
            .bind(("user", user))
            .bind(("status", status))
            .await?;

        Ok(())
    }

    async fn create(&self, user: Thing, post: Thing, status: u8) -> AppResult<()> {
        let _ = self
            .client
            .query(format!(
                "RELATE $post->{POST_USER_TABLE_NAME}->$user SET status=$status;"
            ))
            .bind(("post", post))
            .bind(("user", user))
            .bind(("status", status))
            .await?;

        Ok(())
    }

    async fn get(&self, user: Thing, post: Thing) -> AppResult<Option<PostUserStatus>> {
        let mut res = self
            .client
            .query(format!(
                "(SELECT status FROM $user<-{POST_USER_TABLE_NAME} WHERE in=$post)[0].status;"
            ))
            .bind(("post", post.clone()))
            .bind(("user", user.clone()))
            .await?;
        let status = res.take::<Option<PostUserStatus>>(0)?;
        Ok(status)
    }

    async fn remove(&self, user: Thing, posts: Vec<Thing>) -> AppResult<()> {
        let _ = self
            .client
            .query(format!(
                "DELETE $user<-{POST_USER_TABLE_NAME} WHERE $in IN $posts;"
            ))
            .bind(("posts", posts))
            .bind(("user", user))
            .await?
            .check()?;

        Ok(())
    }
}
