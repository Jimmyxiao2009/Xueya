use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Why the roadmap changed itself.
///
/// Every one of these is a real, persisted row — the prototype used to mutate a
/// JavaScript array in place and call it "自适应".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptationKind {
    MisconceptionDetected,
    ReviewInserted,
    DifficultyChanged,
    LessonReplanned,
    NodeUnlocked,
}

impl AdaptationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MisconceptionDetected => "misconception_detected",
            Self::ReviewInserted => "review_inserted",
            Self::DifficultyChanged => "difficulty_changed",
            Self::LessonReplanned => "lesson_replanned",
            Self::NodeUnlocked => "node_unlocked",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "review_inserted" => Self::ReviewInserted,
            "difficulty_changed" => Self::DifficultyChanged,
            "lesson_replanned" => Self::LessonReplanned,
            "node_unlocked" => Self::NodeUnlocked,
            _ => Self::MisconceptionDetected,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AdaptationEvent {
    pub id: Uuid,
    pub course_id: Uuid,
    pub node_id: Option<Uuid>,
    pub kind: String,
    pub reason: String,
    pub payload_json: String,
    pub created_at: DateTime<Utc>,
}

impl AdaptationEvent {
    pub fn new(
        course_id: Uuid,
        node_id: Option<Uuid>,
        kind: AdaptationKind,
        reason: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            course_id,
            node_id,
            kind: kind.as_str().to_owned(),
            reason: reason.into(),
            payload_json: payload.to_string(),
            created_at: crate::util::time::utc_now(),
        }
    }

    pub fn kind(&self) -> AdaptationKind {
        AdaptationKind::parse(&self.kind)
    }
}

/// A review node injected right after the node the learner just failed.
///
/// The prototype hard-coded `「加练 · 系数与线性组合」`; this produces the same
/// shape of node from real learner data.
pub fn review_node_title(failed_title: &str) -> String {
    let core = failed_title
        .split(['·', '|', ':'])
        .next()
        .unwrap_or(failed_title)
        .trim();
    format!("加练 · {core}")
}

pub fn review_node_objective(misconceptions: &[String]) -> String {
    if misconceptions.is_empty() {
        "针对刚才的薄弱点，再巩固一下。".to_owned()
    } else {
        format!("针对「{}」重新梳理一遍。", misconceptions[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_title_is_derived_from_the_failed_node() {
        assert_eq!(review_node_title("方向与无穷远点"), "加练 · 方向与无穷远点");
        assert_eq!(review_node_title("第一阶段 · 小试身手"), "加练 · 第一阶段");
    }

    #[test]
    fn adaptation_kind_round_trips() {
        for kind in [
            AdaptationKind::MisconceptionDetected,
            AdaptationKind::ReviewInserted,
            AdaptationKind::DifficultyChanged,
            AdaptationKind::LessonReplanned,
            AdaptationKind::NodeUnlocked,
        ] {
            assert_eq!(AdaptationKind::parse(kind.as_str()), kind);
        }
    }
}
