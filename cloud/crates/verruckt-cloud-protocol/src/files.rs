use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresignUploadRequest {
    pub sha256: String,
    pub size: i64,
    pub mime: String,
    #[serde(default)]
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresignUploadResponse {
    pub file_id: Uuid,
    pub object_key: String,
    pub upload_url: String,
    pub method: String,
    pub headers: std::collections::BTreeMap<String, String>,
    pub expires_in: i64,
    pub deduplicated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresignDownloadRequest {
    pub file_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresignDownloadResponse {
    pub file_id: Uuid,
    pub object_key: String,
    pub download_url: String,
    pub method: String,
    pub expires_in: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDto {
    pub id: Uuid,
    pub sha256: String,
    pub size: i64,
    pub mime: String,
    pub filename: Option<String>,
    pub object_key: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFileResponse {
    pub deleted: bool,
    pub file_id: Uuid,
}
