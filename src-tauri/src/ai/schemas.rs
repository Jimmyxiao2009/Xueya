use crate::error::AppError;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoursePlan {
    pub title: String,
    pub description: String,
    pub learning_objectives: Vec<String>,
    pub prerequisites: Vec<String>,
    pub nodes: Vec<CoursePlanNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoursePlanNode {
    pub id: String,
    pub title: String,
    pub objective: String,
    pub description: String,
    pub concepts: Vec<String>,
    pub dependencies: Vec<String>,
    pub evidence_chunk_ids: Vec<Uuid>,
    pub estimated_minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lesson {
    pub title: String,
    pub explanation: String,
    pub key_points: Vec<String>,
    pub diagnostic_question: String,
    pub citations: Vec<LessonCitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonCitation {
    pub chunk_id: Uuid,
    pub quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evaluation {
    pub feedback: String,
    pub correctness: f64,
    pub misconceptions: Vec<String>,
    pub mastery_delta: f64,
    pub follow_up_question: Option<String>,
    pub suggested_action: String,
    /// Optional free-text fields. They default so older providers and tests
    /// remain valid.
    #[serde(default)]
    pub verdict: Option<String>,
    #[serde(default)]
    pub strengths: Vec<String>,
    #[serde(default)]
    pub gaps: Vec<String>,
    #[serde(default)]
    pub diagnoses: Vec<crate::domain::Diagnosis>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeTextEvaluation {
    pub verdict: String,
    pub feedback: String,
    #[serde(default)]
    pub strengths: Vec<String>,
    #[serde(default)]
    pub gaps: Vec<String>,
    #[serde(default)]
    pub diagnoses: Vec<crate::domain::Diagnosis>,
    pub confidence: f64,
    pub mastery_delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizGeneration {
    pub questions: Vec<GeneratedQuizQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedQuizQuestion {
    pub kind: String,
    pub prompt: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub answers: Vec<String>,
    #[serde(default)]
    pub explanation: String,
    #[serde(default = "half")]
    pub difficulty: f64,
}

fn half() -> f64 {
    0.5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorTurn {
    pub message: String,
    #[serde(default)]
    pub citations: Vec<String>,
    #[serde(default)]
    pub suggested_action: String,
    #[serde(default)]
    pub grounding: Option<String>,
    #[serde(default)]
    pub diagnosis: Option<crate::domain::Diagnosis>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSummary {
    pub summary: String,
    #[serde(default)]
    pub key_points: Vec<String>,
}

pub fn parse_json<T: DeserializeOwned>(content: &str) -> Result<T, AppError> {
    let trimmed = content.trim();
    let without_fence = if trimmed.starts_with("```") {
        let trimmed = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```");
        trimmed.trim_end_matches("```").trim()
    } else {
        trimmed
    };
    if let Ok(value) = serde_json::from_str(without_fence) {
        return Ok(value);
    }
    // Real reasoning models sometimes wrap the object in prose or echo a
    // reasoning preamble. Recover the outermost JSON object/array instead of
    // failing the whole teaching turn.
    if let Some(extracted) = extract_json_object(without_fence) {
        if let Ok(value) = serde_json::from_str(&extracted) {
            return Ok(value);
        }
    }
    serde_json::from_str(without_fence)
        .map_err(|error| AppError::AiResponseInvalid(format!("invalid JSON: {error}")))
}

/// Return the first balanced JSON object or array in `content`, ignoring
/// braces inside JSON strings. Scans bytes but only slices on char boundaries.
fn extract_json_object(content: &str) -> Option<String> {
    let bytes = content.as_bytes();
    let start = bytes.iter().position(|byte| *byte == b'{' || *byte == b'[')?;
    let open = bytes[start];
    let close = if open == b'{' { b'}' } else { b']' };
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in bytes[start..].iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => in_string = true,
            value if value == open => depth += 1,
            value if value == close => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = start + offset + 1;
                    return Some(content[start..end].to_owned());
                }
            }
            _ => {}
        }
    }
    None
}

pub fn validate_course_plan(
    plan: &CoursePlan,
    known_chunk_ids: &HashSet<Uuid>,
) -> Result<(), AppError> {
    if plan.title.trim().is_empty() {
        return Err(AppError::AiResponseInvalid("course title is empty".into()));
    }
    if plan.nodes.is_empty() {
        return Err(AppError::AiResponseInvalid("course has no nodes".into()));
    }

    let mut node_ids = HashSet::new();
    for node in &plan.nodes {
        if node.id.trim().is_empty() {
            return Err(AppError::AiResponseInvalid(
                "course node id is empty".into(),
            ));
        }
        if !node_ids.insert(node.id.clone()) {
            return Err(AppError::AiResponseInvalid(format!(
                "duplicate course node id: {}",
                node.id
            )));
        }
        if node.title.trim().is_empty() || node.objective.trim().is_empty() {
            return Err(AppError::AiResponseInvalid(format!(
                "course node {} is missing title or objective",
                node.id
            )));
        }
        if node.estimated_minutes <= 0 {
            return Err(AppError::AiResponseInvalid(format!(
                "course node {} has invalid estimated minutes",
                node.id
            )));
        }
        // With local evidence, every node must cite at least one real chunk.
        // Without local evidence (topic-only generation), empty citations are
        // expected — but the model must not invent chunk ids.
        if known_chunk_ids.is_empty() {
            if !node.evidence_chunk_ids.is_empty() {
                return Err(AppError::AiResponseInvalid(format!(
                    "course node {} references evidence chunks but no local evidence was provided",
                    node.id
                )));
            }
        } else {
            if node.evidence_chunk_ids.is_empty() {
                return Err(AppError::AiResponseInvalid(format!(
                    "course node {} has no evidence chunks",
                    node.id
                )));
            }
            for chunk_id in &node.evidence_chunk_ids {
                if !known_chunk_ids.contains(chunk_id) {
                    return Err(AppError::AiResponseInvalid(format!(
                        "course node {} references unknown evidence chunk {}",
                        node.id, chunk_id
                    )));
                }
            }
        }
    }

    for node in &plan.nodes {
        for dependency in &node.dependencies {
            if !node_ids.contains(dependency) {
                return Err(AppError::AiResponseInvalid(format!(
                    "course node {} depends on unknown node {}",
                    node.id, dependency
                )));
            }
        }
    }

    if has_cycle(&plan.nodes) {
        return Err(AppError::AiResponseInvalid(
            "course dependency graph contains a cycle".into(),
        ));
    }

    Ok(())
}

fn has_cycle(nodes: &[CoursePlanNode]) -> bool {
    let indexes: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect();
    let mut visiting = vec![false; nodes.len()];
    let mut visited = vec![false; nodes.len()];

    fn visit(
        index: usize,
        nodes: &[CoursePlanNode],
        indexes: &HashMap<&str, usize>,
        visiting: &mut [bool],
        visited: &mut [bool],
    ) -> bool {
        if visiting[index] {
            return true;
        }
        if visited[index] {
            return false;
        }
        visiting[index] = true;
        for dependency in &nodes[index].dependencies {
            if let Some(next) = indexes.get(dependency.as_str()) {
                if visit(*next, nodes, indexes, visiting, visited) {
                    return true;
                }
            }
        }
        visiting[index] = false;
        visited[index] = true;
        false
    }

    for index in 0..nodes.len() {
        if visit(index, nodes, &indexes, &mut visiting, &mut visited) {
            return true;
        }
    }
    false
}

pub fn validate_lesson_citations(
    lesson: &Lesson,
    known_chunk_ids: &HashSet<Uuid>,
) -> Result<(), AppError> {
    if lesson.explanation.trim().is_empty() {
        return Err(AppError::AiResponseInvalid(
            "lesson explanation is empty".into(),
        ));
    }
    if lesson.diagnostic_question.trim().is_empty() {
        return Err(AppError::AiResponseInvalid(
            "lesson diagnostic question is empty".into(),
        ));
    }
    if known_chunk_ids.is_empty() {
        // Topic-only lessons are grounded in the model's general knowledge.
        // They must not fabricate local citations.
        if !lesson.citations.is_empty() {
            return Err(AppError::AiResponseInvalid(
                "lesson cites evidence chunks but no local evidence was provided".into(),
            ));
        }
    } else {
        if lesson.citations.is_empty() {
            return Err(AppError::AiResponseInvalid(
                "lesson has no citations".into(),
            ));
        }
        for citation in &lesson.citations {
            if !known_chunk_ids.contains(&citation.chunk_id) {
                return Err(AppError::AiResponseInvalid(format!(
                    "lesson cites unknown chunk {}",
                    citation.chunk_id
                )));
            }
        }
    }
    Ok(())
}

pub fn sanitize_evaluation(mut evaluation: Evaluation) -> Result<Evaluation, AppError> {
    if !evaluation.correctness.is_finite() {
        return Err(AppError::AiResponseInvalid(
            "evaluation correctness is not finite".into(),
        ));
    }
    if !evaluation.mastery_delta.is_finite() {
        return Err(AppError::AiResponseInvalid(
            "evaluation masteryDelta is not finite".into(),
        ));
    }
    evaluation.correctness = evaluation.correctness.clamp(0.0, 1.0);
    evaluation.mastery_delta = evaluation.mastery_delta.clamp(-0.25, 0.25);
    evaluation.misconceptions.truncate(20);
    for diagnosis in &mut evaluation.diagnoses {
        if !diagnosis.confidence.is_finite() || !diagnosis.severity.is_finite() {
            return Err(AppError::AiResponseInvalid(
                "evaluation diagnosis has a non-finite confidence or severity".into(),
            ));
        }
        diagnosis.confidence = diagnosis.confidence.clamp(0.0, 1.0);
        diagnosis.severity = diagnosis.severity.clamp(0.0, 1.0);
    }
    Ok(evaluation)
}

pub fn validate_free_text_evaluation(evaluation: &FreeTextEvaluation) -> Result<(), AppError> {
    if evaluation.feedback.trim().is_empty() {
        return Err(AppError::AiResponseInvalid(
            "free-text evaluation feedback is empty".into(),
        ));
    }
    if !evaluation.confidence.is_finite() || !evaluation.mastery_delta.is_finite() {
        return Err(AppError::AiResponseInvalid(
            "free-text evaluation has non-finite numbers".into(),
        ));
    }
    match evaluation.verdict.trim().to_ascii_lowercase().as_str() {
        "correct" | "partial" | "incorrect" | "uncertain" => {}
        other => {
            return Err(AppError::AiResponseInvalid(format!(
                "unknown free-text verdict: {other}"
            )))
        }
    }
    for diagnosis in &evaluation.diagnoses {
        if !diagnosis.confidence.is_finite() || !diagnosis.severity.is_finite() {
            return Err(AppError::AiResponseInvalid(
                "diagnosis has a non-finite confidence or severity".into(),
            ));
        }
    }
    Ok(())
}

pub fn validate_quiz_generation(generation: &QuizGeneration) -> Result<(), AppError> {
    if generation.questions.is_empty() {
        return Err(AppError::AiResponseInvalid(
            "quiz generation produced no questions".into(),
        ));
    }
    for (index, question) in generation.questions.iter().enumerate() {
        if question.prompt.trim().is_empty() {
            return Err(AppError::AiResponseInvalid(format!(
                "quiz question {index} has an empty prompt"
            )));
        }
        match question.kind.as_str() {
            "single" => {
                if question.options.is_empty() {
                    return Err(AppError::AiResponseInvalid(format!(
                        "single-choice question {index} has no options"
                    )));
                }
                if question.answers.len() != 1 {
                    return Err(AppError::AiResponseInvalid(format!(
                        "single-choice question {index} must have exactly one answer"
                    )));
                }
                if !option_contains(&question.options, &question.answers[0]) {
                    return Err(AppError::AiResponseInvalid(format!(
                        "single-choice question {index} answer is not one of its options"
                    )));
                }
            }
            "multi" => {
                if question.options.is_empty() {
                    return Err(AppError::AiResponseInvalid(format!(
                        "multi-choice question {index} has no options"
                    )));
                }
                if question.answers.is_empty() {
                    return Err(AppError::AiResponseInvalid(format!(
                        "multi-choice question {index} must have at least one answer"
                    )));
                }
                for answer in &question.answers {
                    if !option_contains(&question.options, answer) {
                        return Err(AppError::AiResponseInvalid(format!(
                            "multi-choice question {index} answer is not one of its options"
                        )));
                    }
                }
            }
            "fill" => {
                if question.answers.is_empty() {
                    return Err(AppError::AiResponseInvalid(format!(
                        "fill question {index} must have at least one acceptable answer"
                    )));
                }
            }
            "short" => {}
            other => {
                return Err(AppError::AiResponseInvalid(format!(
                    "unsupported generated quiz kind: {other}"
                )))
            }
        }
        if !question.difficulty.is_finite() {
            return Err(AppError::AiResponseInvalid(format!(
                "quiz question {index} difficulty is not finite"
            )));
        }
    }
    Ok(())
}

fn option_contains(options: &[String], answer: &str) -> bool {
    options
        .iter()
        .any(|option| option == answer || option.trim() == answer.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, dependencies: Vec<&str>, evidence: Vec<Uuid>) -> CoursePlanNode {
        CoursePlanNode {
            id: id.into(),
            title: format!("Node {id}"),
            objective: "Learn something".into(),
            description: "Description".into(),
            concepts: vec!["concept".into()],
            dependencies: dependencies.into_iter().map(str::to_owned).collect(),
            evidence_chunk_ids: evidence,
            estimated_minutes: 20,
        }
    }

    #[test]
    fn accepts_valid_dag_and_rejects_cycle() {
        let chunk_id = Uuid::new_v4();
        let known: HashSet<Uuid> = [chunk_id].into_iter().collect();
        let valid = CoursePlan {
            title: "Course".into(),
            description: "Description".into(),
            learning_objectives: vec!["Objective".into()],
            prerequisites: vec![],
            nodes: vec![
                node("a", vec![], vec![chunk_id]),
                node("b", vec!["a"], vec![chunk_id]),
            ],
        };
        validate_course_plan(&valid, &known).expect("valid plan");

        let cycle = CoursePlan {
            nodes: vec![
                node("a", vec!["b"], vec![chunk_id]),
                node("b", vec!["a"], vec![chunk_id]),
            ],
            ..valid
        };
        assert!(validate_course_plan(&cycle, &known).is_err());
    }

    #[test]
    fn rejects_unknown_evidence_and_duplicate_nodes() {
        let known = HashSet::new();
        let plan = CoursePlan {
            title: "Course".into(),
            description: "Description".into(),
            learning_objectives: vec![],
            prerequisites: vec![],
            nodes: vec![node("a", vec![], vec![Uuid::new_v4()])],
        };
        assert!(validate_course_plan(&plan, &known).is_err());

        let duplicate = CoursePlan {
            title: "Course".into(),
            description: "Description".into(),
            learning_objectives: vec![],
            prerequisites: vec![],
            nodes: vec![
                node("a", vec![], vec![Uuid::new_v4()]),
                node("a", vec![], vec![Uuid::new_v4()]),
            ],
        };
        assert!(validate_course_plan(&duplicate, &HashSet::new()).is_err());
    }

    #[test]
    fn accepts_topic_only_plan_without_evidence() {
        let plan = CoursePlan {
            title: "Course".into(),
            description: "Description".into(),
            learning_objectives: vec![],
            prerequisites: vec![],
            nodes: vec![node("a", vec![], vec![]), node("b", vec!["a"], vec![])],
        };
        validate_course_plan(&plan, &HashSet::new()).expect("topic-only plan");
    }

    #[test]
    fn parses_fenced_json() {
        let value: serde_json::Value = parse_json("```json\n{\"ok\":true}\n```").expect("json");
        assert_eq!(value["ok"], true);
    }

    #[test]
    fn recovers_json_from_prose_and_reasoning_preambles() {
        let value: serde_json::Value =
            parse_json("好的，这是结果：{\"message\": \"你好\", \"ok\": {\"n\": 1}} 请参考")
                .expect("prose-wrapped json");
        assert_eq!(value["message"], "你好");
        let fenced: serde_json::Value =
            parse_json("```json\n{\"message\": \"带\\\"引号\\\"的文本\"}\n```").expect("fenced");
        assert_eq!(fenced["message"], "带\"引号\"的文本");
        let value: serde_json::Value = parse_json("{\"message\": \"解释：{不是JSON}\"}").expect("braces in strings");
        assert_eq!(value["message"], "解释：{不是JSON}");
    }
}
