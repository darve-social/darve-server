use crate::interfaces::file_storage::FileStorageInterface;
use async_trait::async_trait;
use google_cloud_storage::{
    client::{Client, ClientConfig},
    http::objects::{
        download::Range,
        get::GetObjectRequest,
        upload::{Media, UploadObjectRequest, UploadType},
    },
};

pub struct GoogleCloudFileStorage {
    client: Client,
    bucket: String,
}

impl GoogleCloudFileStorage {
    pub fn from_env() -> Self {
        let mut config = ClientConfig::default().anonymous();

        let bucket = std::env::var("GOOGLE_CLOUD_STORAGE_BUCKET")
            .expect("GOOGLE_CLOUD_STORAGE_BUCKET must be set");

        if let Ok(storage_endpoint) = std::env::var("GOOGLE_CLOUD_STORAGE_ENDPOINT") {
            config.storage_endpoint = storage_endpoint
        }

        if let Ok(project_id) = std::env::var("GOOGLE_CLOUD_STORAGE_PROJECT_ID") {
            config.project_id = Some(project_id);
        }

        GoogleCloudFileStorage {
            bucket,
            client: Client::new(config),
        }
    }
}

#[async_trait]
impl FileStorageInterface for GoogleCloudFileStorage {
    async fn upload(
        &self,
        bytes: Vec<u8>,
        path: Option<&str>,
        file_name: &str,
        content_type: Option<&str>,
    ) -> Result<String, String> {
        let object_name = if path.is_none() || path.unwrap().is_empty() {
            file_name.to_string()
        } else {
            format!("{}/{}", path.unwrap(), file_name)
        };

        let req = UploadObjectRequest {
            bucket: self.bucket.clone(),
            ..Default::default()
        };

        let upload_type = UploadType::Simple(Media {
            name: object_name.to_string().into(),
            content_type: content_type
                .unwrap_or("application/octet-stream")
                .to_string()
                .into(),
            content_length: Some(bytes.len() as u64),
        });

        let obj = self
            .client
            .upload_object(&req, bytes, &upload_type)
            .await
            .map_err(|e| e.to_string())?;

        Ok(obj.media_link)
    }

    async fn download(&self, path: Option<&str>, file_name: &str) -> Result<Vec<u8>, String> {
        let object_name = if path.is_none() || path.unwrap().is_empty() {
            file_name.to_string()
        } else {
            format!("{}/{}", path.unwrap(), file_name)
        };

        let request_type = GetObjectRequest {
            bucket: self.bucket.clone(),
            object: object_name,
            ..Default::default()
        };

        let data = self
            .client
            .download_object(&request_type, &Range::default())
            .await
            .map_err(|e| e.to_string())?;

        Ok(data)
    }
}
