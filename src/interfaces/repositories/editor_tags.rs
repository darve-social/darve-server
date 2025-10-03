use async_trait::async_trait;

use crate::{entities::tag::EditorTag, middleware::error::AppResult};

#[async_trait]
pub trait EditorTagsRepositoryInterface {
    async fn get(&self) -> AppResult<Vec<EditorTag>>;
}
