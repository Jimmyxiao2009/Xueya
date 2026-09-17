use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Document {
    pub id: Uuid,
    pub source_id: Uuid,
    pub revision_id: Uuid,
    pub title: String,
    pub text_content: String,
    pub page_count: i64,
    pub metadata_json: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Chunk {
    pub id: Uuid,
    pub document_id: Uuid,
    pub ordinal: i64,
    pub text: String,
    pub char_start: i64,
    pub char_end: i64,
    pub page_number: Option<i64>,
    pub heading: Option<String>,
    pub token_estimate: i64,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
}
