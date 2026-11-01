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

/// The teaching move an agent used in one conversation turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentActionKind {
    Explain,
    Ask,
    Probe,
    Hint,
    Example,
    Counterexample,
    Challenge,
    Reflect,
    Summarize,
    Transition,
    Exercise,
    Quiz,
    CompleteConcept,
}

impl AgentActionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Explain => "explain",
            Self::Ask => "ask",
            Self::Probe => "probe",
            Self::Hint => "hint",
            Self::Example => "example",
            Self::Counterexample => "counterexample",
            Self::Challenge => "challenge",
            Self::Reflect => "reflect",
            Self::Summarize => "summarize",
            Self::Transition => "transition",
            Self::Exercise => "exercise",
            Self::Quiz => "quiz",
            Self::CompleteConcept => "complete_concept",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "ask" | "question" => Self::Ask,
            "probe" | "diagnose" => Self::Probe,
            "hint" => Self::Hint,
            "example" => Self::Example,
            "counterexample" => Self::Counterexample,
            "challenge" => Self::Challenge,
            "reflect" | "summary" => Self::Reflect,
            "summarize" => Self::Summarize,
            "transition" => Self::Transition,
            "exercise" | "practice" => Self::Exercise,
            "quiz" | "assessment" => Self::Quiz,
            "complete_concept" | "complete" => Self::CompleteConcept,
            _ => Self::Explain,
        }
    }
}

/// A single agent teaching action inside a learning session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAction {
    pub kind: AgentActionKind,
    pub suggested_action: String,
}

/// The learner's state inferred from a conversational response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearnerInference {
    Understood,
    PartiallyUnderstood,
    Misconception,
    Guessing,
    Uncertain,
    NeedsExample,
    NeedsPrerequisite,
    StrongUnderstanding,
}

/// A learner response paired with the agent action that followed it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnerResponse {
    pub text: String,
    pub inferred_state: Option<LearnerInference>,
}

/// A persisted conversational turn: learner response plus agent action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningTurn {
    pub id: Uuid,
    pub session_id: Uuid,
    pub agent_action: AgentAction,
    pub learner_response: Option<LearnerResponse>,
    pub created_at: DateTime<Utc>,
}

/// Raw persisted message row used to assemble [`LearningTurn`] values.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LearningMessageRecord {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: String,
    pub content: String,
    pub metadata_json: String,
    pub created_at: DateTime<Utc>,
}

/// Continuous internal concept mastery; not a visible score screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConceptState {
    pub concept: String,
    pub mastery: f64,
    pub evidence_count: i64,
    pub updated_at: DateTime<Utc>,
}

/// One piece of evidence that moved mastery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasteryEvidence {
    pub node_id: Uuid,
    pub source: String,
    pub delta: f64,
    pub confidence: f64,
    pub observed_at: DateTime<Utc>,
}