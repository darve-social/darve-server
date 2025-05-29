use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

use async_trait::async_trait;

use crate::interfaces::file_storage::FileStorageInterface;

pub struct LocalFileStorage {
    uploads_dir: String,
    upload_base_url: String,
}

impl LocalFileStorage {
    pub fn new(uploads_dir: String, upload_base_url: String) -> Self {
        LocalFileStorage {
            uploads_dir,
            upload_base_url,
        }
    }

    fn ensure_dir_exists(&self, dir_path: &str) -> std::io::Result<()> {
        let path = Path::new(dir_path);
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }
}

#[async_trait]
impl FileStorageInterface for LocalFileStorage {
    async fn upload(
        &self,
        bytes: Vec<u8>,
        path: Option<&str>,
        file_name: &str,
        _: Option<&str>,
    ) -> Result<String, String> {
        let object_name = if path.is_none() || path.unwrap().is_empty() {
            self.ensure_dir_exists(&self.uploads_dir)
                .map_err(|e| e.to_string())?;
            file_name.to_string()
        } else {
            self.ensure_dir_exists(&format!("{}/{}", self.uploads_dir, path.unwrap()))
                .map_err(|e| e.to_string())?;
            format!("{}/{}", path.unwrap(), file_name)
        };

        let path = format!("{}/{}", self.uploads_dir, object_name);
        let mut file = File::create(&path).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;

        Ok(format!("{}/{}", self.upload_base_url, object_name))
    }

    async fn download(&self, path: Option<&str>, file_name: &str) -> Result<Vec<u8>, String> {
        let object_name = if path.is_none() || path.unwrap().is_empty() {
            file_name.to_string()
        } else {
            format!("{}/{}", path.unwrap(), file_name)
        };
        let mut file = File::open(&object_name).map_err(|e| e.to_string())?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
        Ok(buffer)
    }
}
