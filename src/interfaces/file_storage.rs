use async_trait::async_trait;

#[async_trait]
pub trait FileStorageInterface {
    async fn upload(
        &self,
        bytes: Vec<u8>,
        path: Option<&str>,
        file_name: &str,
        content_type: Option<&str>,
    ) -> Result<String, String>;

    async fn download(&self, path: Option<&str>, file_name: &str) -> Result<Vec<u8>, String>;
    async fn delete(&self, path: Option<&str>, file_name: &str) -> Result<(), String>;
}

use std::sync::Arc;

#[async_trait]
impl<T: FileStorageInterface + Send + Sync> FileStorageInterface for Arc<T> {
    async fn upload(
        &self,
        bytes: Vec<u8>,
        path: Option<&str>,
        file_name: &str,
        content_type: Option<&str>,
    ) -> Result<String, String> {
        (**self).upload(bytes, path, file_name, content_type).await
    }

    async fn download(&self, path: Option<&str>, file_name: &str) -> Result<Vec<u8>, String> {
        (**self).download(path, file_name).await
    }

    async fn delete(&self, path: Option<&str>, file_name: &str) -> Result<(), String> {
        (**self).delete(path, file_name).await
    }
}
