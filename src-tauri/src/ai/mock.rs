//! The offline AI provider.
//!
//! Verruckt is BYOK: when no provider is configured (or the key is missing) the
//! registry hands out this provider instead of failing. It never contacts the
//! network, it never invents facts, and it always answers in the same JSON
//! schema the real providers are asked for, so the entire product loop —
//! roadmap, lesson, quiz, diagnosis, adaptation — stays demonstrable offline.
//!
//! It is deliberately *not* presented as a real model: every artefact it
//! produces is tagged `generatedBy = "mock"` and the UI says so.

use super::prompts::{COURSE_PLAN_SYSTEM, EVALUATOR_SYSTEM, LESSON_SYSTEM, TUTOR_SYSTEM};
use super::traits::{ChatProvider, ChatRequest, ChatResponse, EmbeddingProvider};
use crate::curriculum;
use crate::error::AppError;
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

pub const MOCK_MODEL: &str = "orbit-mock-v1";
pub const EMBEDDING_DIMENSIONS: usize = 256;

#[derive(Debug, Clone, Default)]
pub struct MockProvider;

impl MockProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ChatProvider for MockProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, AppError> {
        let system = request
            .messages
            .iter()
            .find(|message| message.role == "system")
            .map(|message| message.content.as_str())
            .unwrap_or_default();
        let user = request
            .messages
            .iter()
            .rev()
            .find(|message| message.role == "user")
            .map(|message| message.content.as_str())
            .unwrap_or_default();

        let content = if system == COURSE_PLAN_SYSTEM {
            course_plan_json(user)?
        } else if system == LESSON_SYSTEM {
            lesson_json(user)?
        } else if system == EVALUATOR_SYSTEM {
            evaluation_json(user)?
        } else if system == TUTOR_SYSTEM {
            tutor_json(user)?
        } else {
            json!({
                "message": "这是离线 Mock Provider 的回答。配置真实 AI Provider 后会替换为模型输出。",
                "suggestedAction": "continue",
            })
            .to_string()
        };

        Ok(ChatResponse {
            content,
            model: MOCK_MODEL.to_owned(),
            usage: None,
        })
    }

    async fn test_connection(&self) -> Result<(), AppError> {
        Ok(())
    }
}

#[async_trait]
impl EmbeddingProvider for MockProvider {
    /// A deterministic hashed bag-of-words embedding.
    ///
    /// It is not semantically clever, but it is *stable* and it makes local
    /// retrieval, chunk ranking and the citation pipeline exercisable with zero
    /// network access. Real embeddings simply replace it via the registry.
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, AppError> {
        Ok(texts.iter().map(|text| hash_embedding(text)).collect())
    }
}

fn hash_embedding(text: &str) -> Vec<f32> {
    let mut vector = vec![0.0f32; EMBEDDING_DIMENSIONS];
    for token in tokenize(text) {
        let digest = simple_hash(&token);
        let bucket = (digest % EMBEDDING_DIMENSIONS as u64) as usize;
        let sign = if (digest >> 40) & 1 == 0 { 1.0 } else { -1.0 };
        vector[bucket] += sign;
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if character.is_alphanumeric() {
            current.push(character.to_ascii_lowercase());
        } else {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    // Character bigrams make the hash useful for CJK text, which has no spaces.
    let characters: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    for window in characters.windows(2) {
        tokens.push(window.iter().collect());
    }
    tokens
}

fn simple_hash(value: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn section<'a>(user: &'a str, label: &str) -> Option<&'a str> {
    let start = user.find(label)? + label.len();
    let rest = &user[start..];
    let end = rest.find('\n').unwrap_or(rest.len());
    Some(rest[..end].trim())
}

fn evidence_chunk_ids(user: &str) -> Vec<Uuid> {
    let mut ids = Vec::new();
    let mut cursor = 0usize;
    while let Some(index) = user[cursor..].find("\"chunkId\":\"") {
        let start = cursor + index + "\"chunkId\":\"".len();
        let Some(end) = user[start..].find('"') else {
            break;
        };
        if let Ok(id) = Uuid::parse_str(&user[start..start + end]) {
            ids.push(id);
        }
        cursor = start + end;
    }
    ids
}

fn course_plan_json(user: &str) -> Result<String, AppError> {
    let topic = section(user, "Topic: ").unwrap_or("新的学习路线");
    let brief = curriculum::RouteBrief::new(topic);
    let plan = curriculum::plan_route(&brief);
    let chunk_ids = evidence_chunk_ids(user);

    let nodes: Vec<serde_json::Value> = plan
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let evidence: Vec<Uuid> = if chunk_ids.is_empty() {
                Vec::new()
            } else {
                vec![chunk_ids[index % chunk_ids.len()]]
            };
            json!({
                "id": node.key,
                "title": node.title,
                "objective": node.objective,
                "description": node.description,
                "concepts": node.concepts,
                "dependencies": node.dependencies,
                "evidenceChunkIds": evidence,
                "estimatedMinutes": node.estimated_minutes,
            })
        })
        .collect();

    Ok(json!({
        "title": plan.title,
        "description": plan.description,
        "learningObjectives": plan.learning_objectives,
        "prerequisites": plan.prerequisites,
        "nodes": nodes,
    })
    .to_string())
}

fn lesson_json(user: &str) -> Result<String, AppError> {
    let title = section(user, "Node: ").unwrap_or("本节内容");
    let objective = section(user, "Objective: ").unwrap_or("");
    let plan = curriculum::plan_lesson(title, objective, &[], title);
    let chunk_ids = evidence_chunk_ids(user);

    let citations: Vec<serde_json::Value> = chunk_ids
        .iter()
        .take(3)
        .map(|id| {
            json!({
                "chunkId": id,
                "quote": format!("{objective}（来自本地资料片段）"),
            })
        })
        .collect();

    Ok(json!({
        "title": plan.title,
        "explanation": plan.explanation,
        "keyPoints": plan.key_points,
        "diagnosticQuestion": plan.diagnostic_question,
        "citations": citations,
    })
    .to_string())
}

fn evaluation_json(user: &str) -> Result<String, AppError> {
    let explanation = user
        .split("Lesson explanation:\n")
        .nth(1)
        .and_then(|rest| rest.split("\n\nDiagnostic question:").next())
        .unwrap_or("");
    let answer = user
        .split("Learner answer:\n")
        .nth(1)
        .and_then(|rest| rest.split("\n\nCurrent mastery:").next())
        .unwrap_or("")
        .trim();

    let correctness = overlap_score(explanation, answer);
    let misconceptions = if correctness < 0.45 {
        vec!["回答没有命中本课的核心概念，可能是概念混淆或未作答到要点。".to_owned()]
    } else {
        Vec::new()
    };
    let mastery_delta = ((correctness - 0.5) * 0.5).clamp(-0.25, 0.25);
    let feedback = if correctness >= 0.7 {
        "抓住了主要概念。试着再补一个具体例子，把结论说完整。".to_owned()
    } else if correctness >= 0.4 {
        "方向是对的，但还缺关键一环。回看上面的解释，用自己的话再讲一遍。".to_owned()
    } else {
        "这次没有命中要点。已为你在路线中插入一个加练节点，我们换一种方式再过一遍。".to_owned()
    };

    Ok(json!({
        "feedback": feedback,
        "correctness": correctness,
        "misconceptions": misconceptions,
        "masteryDelta": mastery_delta,
        "followUpQuestion": "能用一句话概括你刚才的结论吗？",
        "suggestedAction": if correctness >= 0.7 { "advance" } else { "insert_review" },
    })
    .to_string())
}

fn tutor_json(user: &str) -> Result<String, AppError> {
    let message = user
        .split("Learner message:\n")
        .nth(1)
        .unwrap_or(user)
        .trim();
    let lowered = message.to_lowercase();
    let (reply, action) = if message.is_empty() {
        (
            "我在。可以先把你的问题拆成一句你能回答的话，我们从那里开始。",
            "ask",
        )
    } else if lowered.contains("不懂")
        || lowered.contains("不会")
        || lowered.contains("为什么")
        || lowered.contains("困惑")
        || lowered.contains("confus")
    {
        (
            "我们换个角度：先不看定义，试着举一个最小的例子，把它代进去算一遍。\
             如果你愿意，先告诉我你卡在哪一步——是名词不清楚，还是不知道下一步做什么？",
            "explain",
        )
    } else if lowered.contains("懂") || lowered.contains("明白") || lowered.contains("continue")
    {
        (
            "很好。接下来完成这道检查题，然后点击「继续学习」点亮下一个知识点。",
            "quiz",
        )
    } else {
        (
            "收到。我会根据你的回答继续提问；你也可以随时告诉我哪里还不清楚。",
            "continue",
        )
    };

    Ok(json!({
        "message": reply,
        "suggestedAction": action,
        "diagnosis": serde_json::Value::Null,
    })
    .to_string())
}

/// Share of the explanation's content tokens that appear in the answer.
fn overlap_score(explanation: &str, answer: &str) -> f64 {
    let target = tokenize(explanation);
    if target.is_empty() {
        return 0.5;
    }
    let candidate: std::collections::HashSet<String> = tokenize(answer).into_iter().collect();
    if candidate.is_empty() {
        return 0.0;
    }
    let hits = target
        .iter()
        .filter(|token| candidate.contains(token.as_str()))
        .count();
    let coverage = hits as f64 / target.len() as f64;
    let length_bonus = (answer.chars().count() as f64 / 120.0).clamp(0.0, 0.25);
    (coverage * 1.6 + length_bonus).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::traits::ChatMessage;

    fn request(system: &str, user: &str) -> ChatRequest {
        ChatRequest::json(
            MOCK_MODEL,
            vec![ChatMessage::system(system), ChatMessage::user(user)],
        )
    }

    #[tokio::test]
    async fn produces_a_valid_course_plan_for_the_demo_topics() {
        let provider = MockProvider::new();
        let chunk = Uuid::new_v4();
        let user = format!(
            "Topic: 射影几何\n\nLocal evidence chunks (JSON):\n[{{\"chunkId\":\"{chunk}\",\"pageNumber\":1,\"text\":\"平行线\"}}]\n\nCreate the roadmap now."
        );
        let response = provider
            .chat(request(COURSE_PLAN_SYSTEM, &user))
            .await
            .expect("chat");
        let plan: crate::ai::schemas::CoursePlan =
            crate::ai::schemas::parse_json(&response.content).expect("parse");
        assert_eq!(plan.nodes.len(), 6);
        assert_eq!(plan.nodes[2].title, "方向与无穷远点");
        assert_eq!(plan.nodes[0].evidence_chunk_ids, vec![chunk]);
    }

    #[tokio::test]
    async fn lesson_output_satisfies_the_citation_schema() {
        let provider = MockProvider::new();
        let chunk = Uuid::new_v4();
        let user = format!(
            "Node: 无穷远点\nObjective: 把方向变成一个点\nCurrent mastery: 0.00\nPrevious misconceptions: []\n\nEvidence chunks (JSON):\n[{{\"chunkId\":\"{chunk}\",\"pageNumber\":null,\"text\":\"...\"}}]\n\nCreate the lesson and one diagnostic question."
        );
        let response = provider
            .chat(request(LESSON_SYSTEM, &user))
            .await
            .expect("chat");
        let lesson: crate::ai::schemas::Lesson =
            crate::ai::schemas::parse_json(&response.content).expect("parse");
        assert_eq!(lesson.title, "无穷远点");
        assert_eq!(lesson.citations.len(), 1);
        assert_eq!(lesson.citations[0].chunk_id, chunk);
    }

    #[tokio::test]
    async fn evaluation_rewards_answers_that_use_the_lesson_vocabulary() {
        let provider = MockProvider::new();
        let good = "Lesson explanation:\n平行线在射影平面共享一个无穷远点，方向相同才对应同一个点\n\nDiagnostic question:\n为什么？\n\nLearner answer:\n方向相同的平行线共享一个无穷远点\n\nCurrent mastery: 0.30\n\nEvidence chunks (JSON):\n[]\n\nEvaluate the answer now.";
        let response = provider
            .chat(request(EVALUATOR_SYSTEM, good))
            .await
            .expect("chat");
        let evaluation: crate::ai::schemas::Evaluation =
            crate::ai::schemas::parse_json(&response.content).expect("parse");
        assert!(evaluation.correctness > 0.5);

        let empty = "Lesson explanation:\n平行线在射影平面共享一个无穷远点\n\nDiagnostic question:\n为什么？\n\nLearner answer:\n不知道\n\nCurrent mastery: 0.30\n\nEvidence chunks (JSON):\n[]\n\nEvaluate the answer now.";
        let response = provider
            .chat(request(EVALUATOR_SYSTEM, empty))
            .await
            .expect("chat");
        let evaluation: crate::ai::schemas::Evaluation =
            crate::ai::schemas::parse_json(&response.content).expect("parse");
        assert!(evaluation.correctness < 0.5);
        assert!(!evaluation.misconceptions.is_empty());
    }

    #[tokio::test]
    async fn embeddings_are_stable_and_normalised() {
        let provider = MockProvider::new();
        let first = provider
            .embed(vec!["平行线的方向".to_owned(), "平行线的方向".to_owned()])
            .await
            .expect("embed");
        assert_eq!(first[0], first[1]);
        assert_eq!(first[0].len(), EMBEDDING_DIMENSIONS);
        let norm: f32 = first[0].iter().map(|value| value * value).sum();
        assert!((norm - 1.0).abs() < 1e-4);
    }

    #[test]
    fn section_reader_stops_at_the_newline() {
        assert_eq!(
            section("Node: 无穷远点\nObjective: x", "Node: "),
            Some("无穷远点")
        );
    }
}
