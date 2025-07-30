use crate::entities::community::post_entity::TABLE_NAME as POST_TABLE_NAME;
use crate::entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME;
use crate::{
    database::client::Db,
    middleware::error::{AppError, AppResult},
};
use std::fmt::Debug;
use std::sync::Arc;
use surrealdb::sql::Thing;

#[derive(Debug)]
pub struct ArchivedPostsRepository {
    client: Arc<Db>,
}

impl ArchivedPostsRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self { client }
    }

    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
      DEFINE TABLE IF NOT EXISTS archived_post TYPE RELATION IN {USER_TABLE_NAME} OUT {POST_TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
      DEFINE INDEX IF NOT EXISTS in_out_unique_idx ON archived_post FIELDS in, out UNIQUE;
      DEFINE FIELD IF NOT EXISTS created_at ON TABLE archived_post TYPE datetime DEFAULT time::now();
    ");
        let mutation = self.client.query(sql).await?;

        mutation
            .check()
            .expect("should mutate TaskRelatesRepository");

        Ok(())
    }

    pub async fn archive(&self, post_id: &str, user_id: &str) -> AppResult<()> {
        self.client
            .query("RELATE $user->archived_post->$post;")
            .bind(("user", Thing::from((USER_TABLE_NAME, user_id))))
            .bind(("post", Thing::from((POST_TABLE_NAME, post_id))))
            .await?
            .check()?;

        Ok(())
    }

    pub async fn unarchive(&self, post_id: &str, user_id: &str) -> AppResult<()> {
        self.client
            .query("DELETE $user->archived_post WHERE out=$post;")
            .bind(("user", Thing::from((USER_TABLE_NAME, user_id))))
            .bind(("post", Thing::from((POST_TABLE_NAME, post_id))))
            .await?
            .check()?;

        Ok(())
    }
}
