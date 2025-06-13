use crate::interfaces::file_storage::FileStorageInterface;
use async_trait::async_trait;
use google_cloud_storage::client::google_cloud_auth::credentials::CredentialsFile;
use google_cloud_storage::http::objects::delete::DeleteObjectRequest;
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
    endpoint: String,
}

impl GoogleCloudFileStorage {
    pub async fn new(bucket: &str, cred: Option<&str>, endpoint: Option<&str>) -> Self {
        let mut config = if cred.is_none() {
            println!("GOOGLE_CLOUD_STORAGE_CREDENTIALS filepath not set - going anonymous");
            ClientConfig::default().anonymous()
        } else {
            ClientConfig::default()
                .with_credentials(
                    CredentialsFile::new_from_file(cred.unwrap().to_string())
                        .await
                        .expect("Credentials file not found"),
                )
                .await
                .expect("Failed to load Google Cloud Storage credentials")
        };
        
        let endpoint = endpoint
            .map(|storage_endpoint| {
                config.storage_endpoint = storage_endpoint.to_string();
                format!("{}/download/storage/v1/b/{}/o", storage_endpoint, bucket)
            })
            .unwrap_or_else(|| format!("{}/{}", config.storage_endpoint, bucket));
        
        println!("GOOGLE_CLOUD_STORAGE_ENDPOINT: {}", endpoint);
        
        GoogleCloudFileStorage {
            bucket: bucket.to_string(),
            client: Client::new(config),
            endpoint,
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

        self.client
            .upload_object(&req, bytes, &upload_type)
            .await
            .map_err(|e| e.to_string())?;

        Ok(format!("{}/{}", self.endpoint, object_name))
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

    async fn delete(&self, path: Option<&str>, file_name: &str) -> Result<(), String> {
        let object_name = if path.is_none() || path.unwrap().is_empty() {
            file_name.to_string()
        } else {
            format!("{}/{}", path.unwrap(), file_name)
        };

        let request_type = DeleteObjectRequest {
            bucket: self.bucket.clone(),
            object: object_name,
            ..Default::default()
        };

        self.client
            .delete_object(&request_type)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
