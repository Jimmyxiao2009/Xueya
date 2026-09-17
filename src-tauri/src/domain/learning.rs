use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum LearningSessionStatus {
    Active,
    Completed,
    Abandoned,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LearningSession {
    pub id: Uuid,
    pub course_id: Uuid,
    pub node_id: Uuid,
    pub status: LearningSessionStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub summary_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LearningAttempt {
    pub id: Uuid,
    pub session_id: Uuid,
    pub node_id: Uuid,
    pub user_answer: String,
    pub evaluation_json: String,
    pub correctness: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MasteryState {
    pub node_id: Uuid,
    pub mastery: f64,
    pub attempts: i64,
    pub updated_at: DateTime<Utc>,
}
