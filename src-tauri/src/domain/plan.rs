use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The learning state machine a node walks through:
/// Explore -> Understand -> Check -> Reinforce -> Complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LessonStepKind {
    Introduction,
    Explanation,
    Question,
    Quiz,
    Reflection,
    Summary,
}

impl LessonStepKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Introduction => "introduction",
            Self::Explanation => "explanation",
            Self::Question => "question",
            Self::Quiz => "quiz",
            Self::Reflection => "reflection",
            Self::Summary => "summary",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "introduction" => Self::Introduction,
            "question" => Self::Question,
            "quiz" => Self::Quiz,
            "reflection" => Self::Reflection,
            "summary" => Self::Summary,
            _ => Self::Explanation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LessonStep {
    pub id: Uuid,
    pub node_id: Uuid,
    pub ordinal: i64,
    pub title: String,
    pub detail: String,
    pub kind: String,
    pub duration_label: String,
    pub created_at: DateTime<Utc>,
}

impl LessonStep {
    pub fn new(
        node_id: Uuid,
        ordinal: i64,
        title: impl Into<String>,
        detail: impl Into<String>,
        kind: LessonStepKind,
        duration_label: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id,
            ordinal,
            title: title.into(),
            detail: detail.into(),
            kind: kind.as_str().to_owned(),
            duration_label: duration_label.into(),
            created_at: crate::util::time::utc_now(),
        }
    }

    pub fn kind(&self) -> LessonStepKind {
        LessonStepKind::parse(&self.kind)
    }
}

/// Which phase of the lesson a session is currently in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LessonPhase {
    Intro,
    Explain,
    Check,
    Reinforce,
    Summary,
    Complete,
}

impl LessonPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Intro => "intro",
            Self::Explain => "explain",
            Self::Check => "check",
            Self::Reinforce => "reinforce",
            Self::Summary => "summary",
            Self::Complete => "complete",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "intro" => Self::Intro,
            "check" => Self::Check,
            "reinforce" => Self::Reinforce,
            "summary" => Self::Summary,
            "complete" => Self::Complete,
            _ => Self::Explain,
        }
    }

    /// Advancing past the last check lands on `Complete`.
    pub fn advance(self, completed_steps: i64, total_steps: i64) -> Self {
        if total_steps > 0 && completed_steps >= total_steps {
            return Self::Complete;
        }
        match self {
            Self::Intro => Self::Explain,
            Self::Explain => Self::Check,
            Self::Check => Self::Reinforce,
            Self::Reinforce => Self::Summary,
            Self::Summary | Self::Complete => Self::Complete,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_walks_the_documented_loop() {
        let mut phase = LessonPhase::Intro;
        for expected in [
            LessonPhase::Explain,
            LessonPhase::Check,
            LessonPhase::Reinforce,
            LessonPhase::Summary,
            LessonPhase::Complete,
        ] {
            phase = phase.advance(1, 10);
            assert_eq!(phase, expected);
        }
        assert_eq!(phase.advance(1, 10), LessonPhase::Complete);
    }

    #[test]
    fn completing_every_step_jumps_to_complete() {
        assert_eq!(LessonPhase::Check.advance(4, 4), LessonPhase::Complete);
    }

    #[test]
    fn step_kind_round_trips() {
        for kind in [
            LessonStepKind::Introduction,
            LessonStepKind::Explanation,
            LessonStepKind::Question,
            LessonStepKind::Quiz,
            LessonStepKind::Reflection,
            LessonStepKind::Summary,
        ] {
            assert_eq!(LessonStepKind::parse(kind.as_str()), kind);
        }
    }
}
