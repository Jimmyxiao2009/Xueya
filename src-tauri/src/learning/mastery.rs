pub fn update_mastery(old_mastery: f64, mastery_delta: f64, attempt_confidence: f64) -> f64 {
    let old_mastery = old_mastery.clamp(0.0, 1.0);
    let mastery_delta = mastery_delta.clamp(-0.25, 0.25);
    let attempt_confidence = attempt_confidence.clamp(0.0, 1.0);
    let headroom = if mastery_delta >= 0.0 {
        1.0 - old_mastery
    } else {
        old_mastery.max(0.1)
    };
    (old_mastery + mastery_delta * attempt_confidence * headroom).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_mastery_to_zero_and_one() {
        assert_eq!(update_mastery(0.0, -0.25, 1.0), 0.0);
        assert_eq!(update_mastery(1.0, 0.25, 1.0), 1.0);
        let value = update_mastery(0.5, 0.2, 1.0);
        assert!((value - 0.6).abs() < 0.0001);
    }

    #[test]
    fn confidence_reduces_extreme_delta() {
        let confident = update_mastery(0.5, 0.2, 1.0);
        let uncertain = update_mastery(0.5, 0.2, 0.25);
        assert!(confident > uncertain);
    }
}
