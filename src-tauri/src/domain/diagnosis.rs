use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single observed learning event, before the adaptation policy decides what
/// to do about it.
///
/// Quiz grading, free-text evaluation and tutor diagnosis all normalise into
/// this type. The policy is then the only place that can change the roadmap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    pub kind: DiagnosisKind,
    #[serde(default)]
    pub knowledge_point_ids: Vec<Uuid>,
    pub confidence: f64,
    pub severity: f64,
    pub explanation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum DiagnosisKind {
    IncorrectAnswer,
    PartialUnderstanding,
    Misconception,
    LowConfidence,
    RepeatedFailure,
    Mastered,
    Uncertain,
}

impl DiagnosisKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IncorrectAnswer => "IncorrectAnswer",
            Self::PartialUnderstanding => "PartialUnderstanding",
            Self::Misconception => "Misconception",
            Self::LowConfidence => "LowConfidence",
            Self::RepeatedFailure => "RepeatedFailure",
            Self::Mastered => "Mastered",
            Self::Uncertain => "Uncertain",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "IncorrectAnswer" | "incorrect_answer" | "incorrect" => Self::IncorrectAnswer,
            "PartialUnderstanding" | "partial_understanding" | "partial" => {
                Self::PartialUnderstanding
            }
            "Misconception" | "misconception" => Self::Misconception,
            "LowConfidence" | "low_confidence" => Self::LowConfidence,
            "RepeatedFailure" | "repeated_failure" => Self::RepeatedFailure,
            "Mastered" | "mastered" => Self::Mastered,
            _ => Self::Uncertain,
        }
    }
}

impl Diagnosis {
    pub fn new(
        kind: DiagnosisKind,
        knowledge_point_ids: Vec<Uuid>,
        confidence: f64,
        severity: f64,
        explanation: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            knowledge_point_ids,
            confidence: confidence.clamp(0.0, 1.0),
            severity: severity.clamp(0.0, 1.0),
            explanation: explanation.into(),
        }
    }

    pub fn with_point(mut self, point_id: Uuid) -> Self {
        if !self.knowledge_point_ids.contains(&point_id) {
            self.knowledge_point_ids.push(point_id);
        }
        self
    }

    pub fn is_actionable(&self) -> bool {
        self.confidence >= 0.55
            && !matches!(
                self.kind,
                DiagnosisKind::Uncertain | DiagnosisKind::Mastered
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptationDecision {
    None,
    InsertReview,
    ScheduleReview,
    IncreasePractice,
    AdjustMastery,
}

/// The single decision table for roadmap adaptation.
///
/// Keeping this pure and inspectable is the point: every caller can be tested
/// with the same diagnosis and must get the same decision.
#[derive(Debug, Clone, Copy, Default)]
pub struct AdaptationPolicy;

impl AdaptationPolicy {
    pub fn decide(&self, diagnosis: &Diagnosis) -> AdaptationDecision {
        if !diagnosis.is_actionable() {
            return AdaptationDecision::None;
        }
        match diagnosis.kind {
            DiagnosisKind::Mastered | DiagnosisKind::Uncertain => AdaptationDecision::None,
            DiagnosisKind::Misconception => AdaptationDecision::InsertReview,
            DiagnosisKind::RepeatedFailure => AdaptationDecision::InsertReview,
            DiagnosisKind::IncorrectAnswer => {
                if diagnosis.severity >= 0.65 {
                    AdaptationDecision::InsertReview
                } else {
                    AdaptationDecision::IncreasePractice
                }
            }
            DiagnosisKind::PartialUnderstanding => {
                if diagnosis.severity >= 0.7 {
                    AdaptationDecision::IncreasePractice
                } else {
                    AdaptationDecision::AdjustMastery
                }
            }
            DiagnosisKind::LowConfidence => AdaptationDecision::ScheduleReview,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_confidence_diagnosis_never_inserts_a_node() {
        let diagnosis = Diagnosis::new(
            DiagnosisKind::Misconception,
            vec![],
            0.3,
            0.9,
            "maybe confused",
        );
        assert_eq!(
            AdaptationPolicy.decide(&diagnosis),
            AdaptationDecision::None
        );
    }

    #[test]
    fn policy_is_deterministic_and_explainable() {
        let point = Uuid::new_v4();
        let diagnosis = Diagnosis::new(
            DiagnosisKind::IncorrectAnswer,
            vec![point],
            0.9,
            0.8,
            "wrong",
        );
        assert_eq!(
            AdaptationPolicy.decide(&diagnosis),
            AdaptationDecision::InsertReview
        );

        let repeated = Diagnosis::new(
            DiagnosisKind::RepeatedFailure,
            vec![point],
            0.8,
            0.6,
            "failed twice",
        );
        assert_eq!(
            AdaptationPolicy.decide(&repeated),
            AdaptationDecision::InsertReview
        );
    }
}
