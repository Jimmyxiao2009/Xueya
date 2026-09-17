use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum CourseStatus {
    Draft,
    Ready,
    Failed,
}

/// A knowledge node in the roadmap.
///
/// `CourseNodeStatus` is the *scheduling* state of the node; `NodeKind` is what
/// the node actually is. The prototype only ever rendered four visual variants
/// (current / completed / review / quiz) so these two enums map onto them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum CourseNodeStatus {
    Locked,
    Available,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum NodeKind {
    Lesson,
    Review,
    Quiz,
    Practice,
}

impl NodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lesson => "lesson",
            Self::Review => "review",
            Self::Quiz => "quiz",
            Self::Practice => "practice",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "review" => Self::Review,
            "quiz" => Self::Quiz,
            "practice" => Self::Practice,
            _ => Self::Lesson,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Course {
    pub id: Uuid,
    pub topic: String,
    pub title: String,
    pub description: String,
    pub status: CourseStatus,
    pub metadata_json: String,
    pub goal: Option<String>,
    pub experience_level: Option<String>,
    pub desired_depth: Option<String>,
    pub kind: String,
    pub current_node_id: Option<Uuid>,
    pub seeded: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CourseNode {
    pub id: Uuid,
    pub course_id: Uuid,
    pub title: String,
    pub description: String,
    pub objective: String,
    pub order_hint: i64,
    pub estimated_minutes: i64,
    pub status: CourseNodeStatus,
    pub metadata_json: String,
    pub lesson_cache_json: Option<String>,
    pub lesson_cache_hash: Option<String>,
    pub kind: String,
    pub confidence: f64,
    pub review_at: Option<DateTime<Utc>>,
    pub generated_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CourseNodeDependency {
    pub course_id: Uuid,
    pub node_id: Uuid,
    pub depends_on_node_id: Uuid,
}

/// Everything the roadmap screen needs in a single round-trip.
#[derive(Debug, Clone)]
pub struct RouteProgress {
    pub total: i64,
    pub completed: i64,
    pub current_node_id: Option<Uuid>,
}

impl RouteProgress {
    pub fn ratio(&self) -> f64 {
        if self.total <= 0 {
            0.0
        } else {
            (self.completed as f64 / self.total as f64).clamp(0.0, 1.0)
        }
    }
}
