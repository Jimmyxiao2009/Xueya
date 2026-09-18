use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncChangeInput {
    pub change_id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub base_revision: i64,
    pub revision: i64,
    pub operation: String,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPushRequest {
    pub device_id: Uuid,
    pub changes: Vec<SyncChangeInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAccepted {
    pub change_id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub revision: i64,
    pub already_applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConflict {
    pub change_id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub server_version: i64,
    pub client_version: i64,
    pub server_payload: Value,
    pub server_updated_at: DateTime<Utc>,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPushResponse {
    pub accepted: Vec<SyncAccepted>,
    pub conflicts: Vec<SyncConflict>,
    pub new_cursor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPullChange {
    pub cursor: String,
    pub device_id: Option<Uuid>,
    pub change_id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub revision: i64,
    pub operation: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPullResponse {
    pub changes: Vec<SyncPullChange>,
    pub next_cursor: String,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEntitySnapshot {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub revision: i64,
    pub deleted: bool,
    pub payload: Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncBootstrapResponse {
    pub entities: Vec<SyncEntitySnapshot>,
    pub next_cursor: String,
    pub has_more: bool,
    pub sync_cursor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAckRequest {
    pub device_id: Uuid,
    pub cursor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAckResponse {
    pub acknowledged_cursor: String,
}
