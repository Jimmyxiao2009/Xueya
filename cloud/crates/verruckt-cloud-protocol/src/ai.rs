use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiUsageEventDto {
    pub id: Uuid,
    pub operation: String,
    pub logical_model: String,
    pub upstream_model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub estimated_cost_units: i64,
    pub status: String,
    pub request_id: String,
    pub latency_ms: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiUsageResponse {
    pub period_start: DateTime<Utc>,
    pub credits_used: i64,
    pub credits_limit: i64,
    pub credits_remaining: i64,
    pub events: Vec<AiUsageEventDto>,
}
