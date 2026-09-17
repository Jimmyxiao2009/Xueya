use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum KnowledgeObjectKind {
    Source,
    Document,
    Chunk,
    Concept,
    Course,
    CourseNode,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum RelationType {
    Contains,
    DerivedFrom,
    Mentions,
    Discusses,
    Supports,
    RelatedTo,
    PrerequisiteOf,
    PartOf,
    AppearsIn,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct KnowledgeObject {
    pub id: Uuid,
    pub kind: KnowledgeObjectKind,
    pub title: String,
    pub description: Option<String>,
    pub metadata_json: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct KnowledgeRelation {
    pub id: Uuid,
    pub from_object_id: Uuid,
    pub to_object_id: Uuid,
    pub relation_type: RelationType,
    pub weight: f64,
    pub metadata_json: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Note {
    pub id: Uuid,
    pub object_id: Uuid,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
