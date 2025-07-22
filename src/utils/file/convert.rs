use std::{fs::File, io::Read, path::Path};

use axum::extract::multipart::Field;
use axum_typed_multipart::FieldData;
use tempfile::NamedTempFile;

use crate::middleware::error::{AppError, AppResult, CtxResult};

pub fn sanitize_filename(file_name: &str) -> String {
    let bad_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    let mut result = file_name.to_owned();
    for &ch in &bad_chars {
        result = result.replace(ch, "_");
    }
    result
}

#[derive(Debug)]
pub struct FileUpload {
    pub content_type: Option<String>,
    pub file_name: String,
    pub data: Vec<u8>,
    pub extension: String,
}

impl FileUpload {
    pub async fn try_from_field(field: Field<'_>) -> AppResult<Self> {
        let file_name = field
            .file_name()
            .ok_or(AppError::Generic {
                description: "Missing file name".to_string(),
            })?
            .to_string();

        let extension = Path::new(&file_name)
            .extension()
            .and_then(|e| e.to_str())
            .ok_or(AppError::Generic {
                description: "File has no valid extension".to_string(),
            })?
            .to_string();

        let content_type = field.content_type().map(|v| v.to_string());

        let data = field.bytes().await.map_err(|e| AppError::Generic {
            description: format!("Failed to read file: {}", e),
        })?;

        Ok(Self {
            file_name,
            content_type,
            data: data.to_vec(),
            extension,
        })
    }
}

pub fn convert_field_file_data(data: FieldData<NamedTempFile<File>>) -> CtxResult<FileUpload> {
    let content_type = data.metadata.content_type;

    let file_name = data.metadata.file_name.expect("file name missing");

    let extension = file_name.split(".").last().ok_or(AppError::Generic {
        description: "File has no extension".to_string(),
    })?;

    let mut buffer = Vec::new();
    let mut file = data.contents.as_file();

    file.read_to_end(&mut buffer)
        .map_err(|e| AppError::Generic {
            description: e.to_string(),
        })?;
    Ok(FileUpload {
        content_type,
        file_name: sanitize_filename(&file_name),
        data: buffer,
        extension: extension.to_string(),
    })
}
