pub type Id = uuid::Uuid;
pub type SourceId = uuid::Uuid;
pub type DocumentId = uuid::Uuid;
pub type ChunkId = uuid::Uuid;
pub type KnowledgeObjectId = uuid::Uuid;
pub type CourseId = uuid::Uuid;
pub type CourseNodeId = uuid::Uuid;
pub type LearningSessionId = uuid::Uuid;
pub type ProviderId = uuid::Uuid;
pub type JobId = uuid::Uuid;

pub fn new_id() -> uuid::Uuid {
    uuid::Uuid::new_v4()
}
