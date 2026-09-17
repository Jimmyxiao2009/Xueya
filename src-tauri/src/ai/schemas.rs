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
    serde_json::from_str(without_fence)
        .map_err(|error| AppError::AiResponseInvalid(format!("invalid JSON: {error}")))
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
    Ok(evaluation)
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
    fn parses_fenced_json() {
        let value: serde_json::Value = parse_json("```json\n{\"ok\":true}\n```").expect("json");
        assert_eq!(value["ok"], true);
    }
}
