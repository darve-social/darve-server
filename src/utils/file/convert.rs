use std::{fs::File, io::Read};

use axum_typed_multipart::FieldData;
use tempfile::NamedTempFile;

use crate::middleware::error::{AppError, CtxResult};

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
