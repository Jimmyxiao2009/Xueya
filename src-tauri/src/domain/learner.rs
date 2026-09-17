use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single concept the learner is expected to acquire. Knowledge points are
/// the unit the learner model tracks; roadmap nodes are the unit the UI shows.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct KnowledgePoint {
    pub id: Uuid,
    pub course_id: Uuid,
    pub title: String,
    pub description: String,
    pub ordinal: i64,
    pub created_at: DateTime<Utc>,
}

/// The minimal usable learner model.
///
/// `mastery` is "how much of this do you know", `confidence` is "how sure are
/// we about that estimate" (it grows with attempts and shrinks with mistakes).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LearnerState {
    pub course_id: Uuid,
    pub knowledge_point_id: Uuid,
    pub confidence: f64,
    pub mastery: f64,
    pub mistakes: i64,
    pub attempts: i64,
    pub last_seen: Option<DateTime<Utc>>,
    pub next_review: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

impl LearnerState {
    pub fn fresh(course_id: Uuid, knowledge_point_id: Uuid) -> Self {
        Self {
            course_id,
            knowledge_point_id,
            confidence: 0.0,
            mastery: 0.0,
            mistakes: 0,
            attempts: 0,
            last_seen: None,
            next_review: None,
            updated_at: crate::util::time::utc_now(),
        }
    }

    pub fn is_review_due(&self, now: DateTime<Utc>) -> bool {
        match self.next_review {
            Some(at) => at <= now,
            None => false,
        }
    }
}

/// Spaced-repetition scheduling for a knowledge point.
///
/// This is deliberately a simple, inspectable Leitner-style ladder instead of a
/// black-box SM-2 clone: the roadmap must stay explainable to the learner.
pub fn next_review_interval_hours(mastery: f64, mistakes: i64) -> i64 {
    let base = match mastery {
        m if m >= 0.9 => 168,
        m if m >= 0.75 => 72,
        m if m >= 0.5 => 24,
        m if m > 0.0 => 8,
        _ => 4,
    };
    (base / (1 + mistakes.max(0)).min(3)).max(2)
}

pub fn schedule_next_review(now: DateTime<Utc>, mastery: f64, mistakes: i64) -> DateTime<Utc> {
    now + chrono::Duration::hours(next_review_interval_hours(mastery, mistakes))
}

/// How a mastery estimate moves after one answer.
///
/// `weight` is the evidence strength (0..1): a multiple-choice guess carries
/// less evidence than a written explanation.
pub fn blend_mastery(previous: f64, score: f64, weight: f64) -> f64 {
    let weight = weight.clamp(0.05, 0.6);
    let updated = previous * (1.0 - weight) + score.clamp(0.0, 1.0) * weight;
    updated.clamp(0.0, 1.0)
}

pub fn blend_confidence(previous: f64, attempts: i64) -> f64 {
    let evidence = (attempts as f64 / (attempts as f64 + 3.0)).clamp(0.0, 1.0);
    (previous * 0.5 + evidence * 0.5).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mastery_moves_towards_the_evidence() {
        let up = blend_mastery(0.2, 1.0, 0.35);
        assert!(up > 0.2 && up < 1.0);
        let down = blend_mastery(0.8, 0.0, 0.35);
        assert!(down < 0.8 && down > 0.0);
        assert!((blend_mastery(0.5, 1.0, 0.0) - 0.525).abs() < 1e-9);
    }

    #[test]
    fn repeated_mistakes_shorten_the_review_interval() {
        assert!(next_review_interval_hours(0.9, 0) > next_review_interval_hours(0.9, 2));
        assert!(next_review_interval_hours(0.0, 0) <= 4);
    }

    #[test]
    fn confidence_grows_with_attempts() {
        let low = blend_confidence(0.0, 1);
        let high = blend_confidence(0.0, 10);
        assert!(high > low);
    }
}
