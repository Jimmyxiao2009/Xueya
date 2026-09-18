use super::mastery::update_mastery;
use crate::ai::schemas::Evaluation;

pub fn attempt_confidence(correctness: f64, answer_chars: usize) -> f64 {
    let length_bonus = ((answer_chars as f64) / 500.0).min(0.2);
    (0.4 + correctness.clamp(0.0, 1.0) * 0.4 + length_bonus).clamp(0.2, 1.0)
}

pub fn next_mastery(old_mastery: f64, evaluation: &Evaluation, answer_chars: usize) -> f64 {
    update_mastery(
        old_mastery,
        evaluation.mastery_delta,
        attempt_confidence(evaluation.correctness, answer_chars),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_and_mastery_are_bounded() {
        let confidence = attempt_confidence(0.8, 120);
        assert!((0.2..=1.0).contains(&confidence));
        let evaluation = Evaluation {
            feedback: "Good".into(),
            correctness: 0.8,
            misconceptions: vec![],
            mastery_delta: 0.2,
            follow_up_question: None,
            suggested_action: "Continue".into(),
            verdict: None,
            strengths: vec![],
            gaps: vec![],
            diagnoses: vec![],
            confidence: None,
        };
        let mastery = next_mastery(0.5, &evaluation, 120);
        assert!((0.0..=1.0).contains(&mastery));
        assert!(mastery > 0.5);
    }
}
