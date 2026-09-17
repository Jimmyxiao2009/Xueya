use crate::domain::JobStatus;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobEvent {
    pub job_id: Uuid,
    pub event: JobEventKind,
    pub status: JobStatus,
    pub progress: f64,
    pub message: String,
    pub error: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobEventKind {
    Started,
    Progress,
    Partial,
    Completed,
    Failed,
    Cancelled,
}
