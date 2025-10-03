use crate::database::client::Db;
use crate::database::table_names::{EDITOR_TAG_TABLE_NAME, TAG_TABLE_NAME};
use crate::entities::tag::EditorTag;
use crate::interfaces::repositories::editor_tags::EditorTagsRepositoryInterface;
use crate::middleware::error::{AppError, AppResult};
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Debug)]
pub struct EditorTagsRepository {
    client: Arc<Db>,
}

impl EditorTagsRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self { client }
    }
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!(
            "
            DEFINE TABLE IF NOT EXISTS {EDITOR_TAG_TABLE_NAME} SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS image_url ON TABLE {EDITOR_TAG_TABLE_NAME} TYPE string;
            DEFINE FIELD IF NOT EXISTS order ON TABLE {EDITOR_TAG_TABLE_NAME} TYPE int;
            DEFINE FIELD IF NOT EXISTS tag ON TABLE {EDITOR_TAG_TABLE_NAME} TYPE record<{TAG_TABLE_NAME}>;
            DEFINE INDEX IF NOT EXISTS idx_order ON {EDITOR_TAG_TABLE_NAME} COLUMNS order;
            "
        );
        let mutation = self.client.query(sql).await?;

        mutation
            .check()
            .expect("should mutate EditorTagsRepository");

        Ok(())
    }
}

#[async_trait]
impl EditorTagsRepositoryInterface for EditorTagsRepository {
    async fn get(&self) -> AppResult<Vec<EditorTag>> {
        let query = format!("SELECT * FROM {EDITOR_TAG_TABLE_NAME} ORDER BY order DESC",);
        let mut res = self.client.query(query).await?;
        let data = res.take::<Vec<EditorTag>>(0)?;
        Ok(data)
    }
}
