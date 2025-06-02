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
