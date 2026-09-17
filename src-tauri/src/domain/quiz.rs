use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The five question widgets the Orbit prototype already ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuizKind {
    Single,
    Multi,
    Fill,
    Short,
    Image,
}

impl QuizKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Multi => "multi",
            Self::Fill => "fill",
            Self::Short => "short",
            Self::Image => "image",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "multi" => Self::Multi,
            "fill" => Self::Fill,
            "short" => Self::Short,
            "image" => Self::Image,
            _ => Self::Single,
        }
    }

    /// Objective widgets can be graded locally and deterministically. Short
    /// answers and photos need the tutor, so they are never auto-claimed as
    /// "correct" by the local engine.
    pub fn is_locally_gradable(self) -> bool {
        matches!(self, Self::Single | Self::Multi | Self::Fill)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct QuizQuestion {
    pub id: Uuid,
    pub node_id: Uuid,
    pub knowledge_point_id: Option<Uuid>,
    pub ordinal: i64,
    pub kind: String,
    pub prompt: String,
    pub options_json: String,
    pub answer_json: String,
    pub explanation: String,
    pub difficulty: f64,
    pub created_at: DateTime<Utc>,
}

impl QuizQuestion {
    pub fn options(&self) -> Vec<String> {
        serde_json::from_str(&self.options_json).unwrap_or_default()
    }

    pub fn expected(&self) -> Vec<String> {
        serde_json::from_str(&self.answer_json).unwrap_or_default()
    }

    pub fn kind(&self) -> QuizKind {
        QuizKind::parse(&self.kind)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct QuizAttempt {
    pub id: Uuid,
    pub question_id: Uuid,
    pub node_id: Uuid,
    pub session_id: Option<Uuid>,
    pub answer_json: String,
    pub correct: i64,
    pub score: f64,
    pub feedback: String,
    pub created_at: DateTime<Utc>,
}

/// The outcome of grading one answer. `graded` is false when the question needs
/// a tutor (or a human) and the local engine refuses to guess.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizGrade {
    pub correct: bool,
    pub graded: bool,
    pub score: f64,
    pub feedback: String,
    pub expected: Vec<String>,
    pub misconceptions: Vec<String>,
}

pub fn normalize_text(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(['.', '。', '!', '！'])
        .replace(' ', "")
        .to_lowercase()
}

/// Local, deterministic grading for single / multi / fill questions.
///
/// `answers` carries option indices as strings for choice widgets and raw text
/// for fill-in-the-blank. Returns `None` when the widget is not locally
/// gradable so the caller can delegate to the tutor.
pub fn grade_locally(question: &QuizQuestion, answers: &[String]) -> Option<QuizGrade> {
    let kind = question.kind();
    if !kind.is_locally_gradable() {
        return None;
    }
    let expected = question.expected();
    if expected.is_empty() {
        return None;
    }

    let (correct, score) = match kind {
        QuizKind::Single => {
            let picked = answers.first().map(String::as_str).unwrap_or_default();
            let ok = normalize_text(picked) == normalize_text(&expected[0]);
            (ok, if ok { 1.0 } else { 0.0 })
        }
        QuizKind::Multi => {
            let mut picked: Vec<String> =
                answers.iter().map(|value| normalize_text(value)).collect();
            let mut wanted: Vec<String> =
                expected.iter().map(|value| normalize_text(value)).collect();
            picked.sort();
            picked.dedup();
            wanted.sort();
            wanted.dedup();
            if picked == wanted {
                (true, 1.0)
            } else {
                let hits = picked.iter().filter(|value| wanted.contains(value)).count();
                let penalty = wanted.len().saturating_sub(hits) + picked.len().saturating_sub(hits);
                (
                    false,
                    (hits as f64 / (hits + penalty).max(1) as f64).clamp(0.0, 0.8),
                )
            }
        }
        QuizKind::Fill => {
            let picked = answers.first().map(String::as_str).unwrap_or_default();
            let ok = expected
                .iter()
                .any(|value| normalize_text(value) == normalize_text(picked));
            (ok, if ok { 1.0 } else { 0.0 })
        }
        _ => return None,
    };

    let feedback = if correct {
        question.explanation.clone()
    } else {
        format!("再想一想。{}", question.explanation)
    };

    Some(QuizGrade {
        correct,
        graded: true,
        score,
        feedback,
        expected,
        misconceptions: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(kind: QuizKind, options: &[&str], answer: &[&str]) -> QuizQuestion {
        QuizQuestion {
            id: Uuid::new_v4(),
            node_id: Uuid::new_v4(),
            knowledge_point_id: None,
            ordinal: 0,
            kind: kind.as_str().to_owned(),
            prompt: "prompt".into(),
            options_json: serde_json::to_string(options).unwrap(),
            answer_json: serde_json::to_string(answer).unwrap(),
            explanation: "explanation".into(),
            difficulty: 0.5,
            created_at: crate::util::time::utc_now(),
        }
    }

    #[test]
    fn single_choice_is_graded_exactly() {
        let q = question(QuizKind::Single, &["a", "b", "c"], &["1"]);
        assert!(grade_locally(&q, &["1".into()]).unwrap().correct);
        assert!(!grade_locally(&q, &["0".into()]).unwrap().correct);
    }

    #[test]
    fn multi_choice_requires_the_exact_set() {
        let q = question(QuizKind::Multi, &["a", "b", "c"], &["0", "2"]);
        assert!(
            grade_locally(&q, &["2".into(), "0".into()])
                .unwrap()
                .correct
        );
        let partial = grade_locally(&q, &["0".into()]).unwrap();
        assert!(!partial.correct);
        assert!(partial.score > 0.0 && partial.score < 1.0);
    }

    #[test]
    fn fill_ignores_case_and_trailing_punctuation() {
        let q = question(QuizKind::Fill, &[], &["0"]);
        assert!(grade_locally(&q, &[" 0。 ".into()]).unwrap().correct);
        assert!(!grade_locally(&q, &["1".into()]).unwrap().correct);
    }

    #[test]
    fn subjective_widgets_are_never_locally_graded() {
        let q = question(QuizKind::Short, &[], &[]);
        assert!(grade_locally(&q, &["anything".into()]).is_none());
        let q = question(QuizKind::Image, &[], &[]);
        assert!(grade_locally(&q, &["data:image/png;base64,aa".into()]).is_none());
    }
}
