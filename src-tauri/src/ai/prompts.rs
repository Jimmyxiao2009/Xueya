use super::traits::ChatMessage;
use crate::error::AppError;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceSnippet {
    pub chunk_id: Uuid,
    pub page_number: Option<i64>,
    pub text: String,
}

pub const COURSE_PLAN_SYSTEM: &str = r#"You are an instructional designer. Build a rigorous, evidence-grounded learning roadmap from the supplied local source chunks.

Return ONLY valid JSON matching this TypeScript type:
{
  "title": string,
  "description": string,
  "learningObjectives": string[],
  "prerequisites": string[],
  "nodes": [
    {
      "id": string,
      "title": string,
      "objective": string,
      "description": string,
      "concepts": string[],
      "dependencies": string[],
      "evidenceChunkIds": string[],
      "estimatedMinutes": number
    }
  ]
}

Rules:
- Node ids must be unique short slugs.
- dependencies must reference ids in this same course.
- evidenceChunkIds must be copied verbatim from provided chunk ids.
- Keep the graph acyclic and ordered from foundations to applications.
- Do not invent facts that are not present in the evidence."#;

pub const LESSON_SYSTEM: &str = r#"You are a patient tutor. Explain one roadmap node using ONLY the supplied local source chunks.

Return ONLY valid JSON matching:
{
  "title": string,
  "explanation": string,
  "keyPoints": string[],
  "diagnosticQuestion": string,
  "citations": [{ "chunkId": string, "quote": string }]
}
Every citation chunkId must be copied from the provided evidence. Every non-trivial claim should be citation-backed."#;

pub const EVALUATOR_SYSTEM: &str = r#"You are a strict but encouraging evaluator. Judge the learner's answer against the supplied node and evidence.

Return ONLY valid JSON matching:
{
  "feedback": string,
  "correctness": number,
  "misconceptions": string[],
  "masteryDelta": number,
  "followUpQuestion": string | null,
  "suggestedAction": string
}
correctness must be between 0 and 1. masteryDelta must be between -0.25 and 0.25. Do not inflate mastery."#;

fn evidence_json(evidence: &[EvidenceSnippet]) -> Result<String, AppError> {
    serde_json::to_string(evidence).map_err(|error| AppError::Internal(error.to_string()))
}

pub fn course_plan_messages(
    topic: &str,
    evidence: &[EvidenceSnippet],
) -> Result<Vec<ChatMessage>, AppError> {
    let evidence = evidence_json(evidence)?;
    Ok(vec![
        ChatMessage::system(COURSE_PLAN_SYSTEM),
        ChatMessage::user(format!(
            "Topic: {topic}\n\nLocal evidence chunks (JSON):\n{evidence}\n\nCreate the roadmap now."
        )),
    ])
}

pub fn lesson_messages(
    title: &str,
    objective: &str,
    mastery: f64,
    previous_misconceptions: &[String],
    evidence: &[EvidenceSnippet],
) -> Result<Vec<ChatMessage>, AppError> {
    let evidence = evidence_json(evidence)?;
    let misconceptions = serde_json::to_string(previous_misconceptions)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(vec![
        ChatMessage::system(LESSON_SYSTEM),
        ChatMessage::user(format!(
            "Node: {title}\nObjective: {objective}\nCurrent mastery: {mastery:.2}\nPrevious misconceptions: {misconceptions}\n\nEvidence chunks (JSON):\n{evidence}\n\nCreate the lesson and one diagnostic question."
        )),
    ])
}

pub fn evaluation_messages(
    node_title: &str,
    explanation: &str,
    diagnostic_question: &str,
    user_answer: &str,
    mastery: f64,
    evidence: &[EvidenceSnippet],
) -> Result<Vec<ChatMessage>, AppError> {
    let evidence = evidence_json(evidence)?;
    Ok(vec![
        ChatMessage::system(EVALUATOR_SYSTEM),
        ChatMessage::user(format!(
            "Node: {node_title}\nLesson explanation:\n{explanation}\n\nDiagnostic question:\n{diagnostic_question}\n\nLearner answer:\n{user_answer}\n\nCurrent mastery: {mastery:.2}\n\nEvidence chunks (JSON):\n{evidence}\n\nEvaluate the answer now."
        )),
    ])
}

/// System prompt for the in-lesson tutor. The mock provider detects this
/// prefix, and real providers receive the same contract.
pub const TUTOR_SYSTEM: &str = r#"You are Orbit, a patient tutor inside a focused learning session.

Return ONLY valid JSON matching:
{
  "message": string,
  "suggestedAction": "explain" | "ask" | "quiz" | "continue" | "advance",
  "diagnosis": string | null
}

Rules:
- Answer in the learner's language.
- Never invent facts that are not in the supplied lesson or evidence.
- If the learner is confused, break the idea down and ask one small question back.
- If the learner is stuck on a specific misconception, name it in "diagnosis"."#;

pub fn tutor_messages(
    node_title: &str,
    explanation: &str,
    plan_summary: &str,
    learner_message: &str,
    evidence: &[EvidenceSnippet],
) -> Result<Vec<ChatMessage>, AppError> {
    let evidence = evidence_json(evidence)?;
    Ok(vec![
        ChatMessage::system(TUTOR_SYSTEM),
        ChatMessage::user(format!(
            "Node: {node_title}\nLesson explanation:\n{explanation}\n\nLearning plan so far:\n{plan_summary}\n\nEvidence chunks (JSON):\n{evidence}\n\nLearner message:\n{learner_message}\n\nReply now."
        )),
    ])
}
