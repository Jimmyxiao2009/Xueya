pub use crate::domain::{LearningSession, LearningSessionStatus};

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub course_id: uuid::Uuid,
    pub node_id: uuid::Uuid,
    pub mastery: f64,
    pub previous_misconceptions: Vec<String>,
    pub evidence_chunk_ids: Vec<uuid::Uuid>,
}
