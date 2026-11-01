//! The explicit offline AI provider.
//!
//! This provider is used only when the learner deliberately selects the
//! offline/local-only AI mode (or in tests and fixtures). Production BYOK and
//! Cloud modes must fail loudly instead of silently falling back here, so the
//! registry never disguises deterministic output as a real model response.
//!
//! It never contacts the network, it never invents facts, and it always answers
//! in the same JSON schema the real providers are asked for, so the entire
//! product loop — roadmap, lesson, quiz, diagnosis, adaptation — stays
//! demonstrable offline. Every artefact it produces is tagged
//! `generatedBy = "mock"` and the UI says so.

use super::prompts::{
    COURSE_PLAN_SYSTEM, EVALUATOR_SYSTEM, FREETEXT_EVALUATION_SYSTEM, LESSON_SYSTEM, QUIZ_SYSTEM,
    SUMMARY_SYSTEM, TUTOR_OPENING_V1_SYSTEM, TUTOR_STREAM_V1_SYSTEM, TUTOR_SYSTEM,
    TUTOR_V1_SYSTEM,
};
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
        } else if system == FREETEXT_EVALUATION_SYSTEM {
            free_text_evaluation_json(user)?
        } else if system == TUTOR_OPENING_V1_SYSTEM {
            tutor_opening_json(user)?
        } else if system == TUTOR_V1_SYSTEM {
            tutor_v1_json(user)?
        } else if system == TUTOR_STREAM_V1_SYSTEM {
            tutor_stream_text(user)
        } else if system == TUTOR_SYSTEM {
            tutor_json(user)?
        } else if system == SUMMARY_SYSTEM {
            summary_json(user)
        } else if system == QUIZ_SYSTEM {
            quiz_json(user)?
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
    // The confusion branch also names a misconception, which is what the
    // roadmap needs in order to adapt. Without a real provider the offline
    // demo would otherwise never reach the diagnosis step.
    let (reply, action, diagnosis): (&str, &str, Option<&str>) = if message.is_empty() {
        (
            "我在。可以先把你的问题拆成一句你能回答的话，我们从那里开始。",
            "ask",
            None,
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
            Some("学习者主动表示不理解，可能停留在名词层面。"),
        )
    } else if lowered.contains("懂") || lowered.contains("明白") || lowered.contains("continue")
    {
        (
            "很好。接下来完成这道检查题，然后点击「继续学习」点亮下一个知识点。",
            "quiz",
            None,
        )
    } else {
        (
            "收到。我会根据你的回答继续提问；你也可以随时告诉我哪里还不清楚。",
            "continue",
            None,
        )
    };

    Ok(json!({
        "message": reply,
        "suggestedAction": action,
        "diagnosis": diagnosis,
    })
    .to_string())
}

fn free_text_evaluation_json(user: &str) -> Result<String, AppError> {
    let explanation = user
        .split("Lesson explanation:\n")
        .nth(1)
        .and_then(|rest| rest.split("\n\nDiagnostic question:").next())
        .unwrap_or("");
    let answer = user
        .split("Learner answer:\n")
        .nth(1)
        .and_then(|rest| rest.split("\n\n").next())
        .unwrap_or("")
        .trim();
    let correctness = overlap_score(explanation, answer);
    let verdict = if correctness >= 0.7 {
        "correct"
    } else if correctness >= 0.4 {
        "partial"
    } else {
        "incorrect"
    };
    let diagnoses = if verdict == "incorrect" {
        vec![json!({
            "kind": "IncorrectAnswer",
            "knowledgePointIds": [],
            "confidence": 0.82,
            "severity": 0.85,
            "explanation": "回答没有命中本课的核心概念。",
        })]
    } else if verdict == "partial" {
        vec![json!({
            "kind": "PartialUnderstanding",
            "knowledgePointIds": [],
            "confidence": 0.72,
            "severity": 0.6,
            "explanation": "方向基本正确，但还缺少一个关键环节。",
        })]
    } else {
        Vec::new()
    };
    let feedback = if verdict == "correct" {
        "抓住了主要概念。再补一个具体例子，把结论说完整。"
    } else if verdict == "partial" {
        "方向是对的，但还缺关键一环。回看上面的解释，用自己的话再讲一遍。"
    } else {
        "这次没有命中要点。已为你在路线中插入一个加练节点，我们换一种方式再过一遍。"
    };
    Ok(json!({
        "verdict": verdict,
        "feedback": feedback,
        "strengths": if correctness >= 0.4 {
            vec!["使用了课程中的部分关键词".to_owned()]
        } else {
            Vec::<String>::new()
        },
        "gaps": if correctness >= 0.7 {
            Vec::<String>::new()
        } else {
            vec!["核心概念之间的因果关系还不够清楚".to_owned()]
        },
        "diagnoses": diagnoses,
        "confidence": 0.8,
        "masteryDelta": ((correctness - 0.5) * 0.5).clamp(-0.25, 0.25),
    })
    .to_string())
}

fn tutor_opening_json(user: &str) -> Result<String, AppError> {
    let title = section(user, "Node: ").unwrap_or("这一节");
    let objective = section(user, "Objective: ").unwrap_or("");
    let key_points = section(user, "Key points: ").unwrap_or("");
    let diagnostic = section(user, "Diagnostic question: ").unwrap_or("");
    let focus = if !diagnostic.is_empty() {
        diagnostic
    } else if !key_points.is_empty() {
        key_points
    } else if !objective.is_empty() {
        objective
    } else {
        "你会怎样向一个完全没学过的人解释它？"
    };
    let message = format!(
        "先不急着记定义。关于「{title}」，我想先请你判断：{focus}\n\n你会怎么解释？"
    );
    Ok(json!({
        "message": message,
        "citations": [],
        "suggestedAction": "ask",
        "grounding": "model_knowledge",
        "diagnosis": null,
    })
    .to_string())
}


/// Deterministic offline tutor move used only when no real provider is configured.
///
/// It keeps the competition demo useful without pretending to be an LLM: the
/// reply is derived from the current node and the learner's last message, and
/// the action still changes across explain / example / hint / probe / transition.
fn tutor_offline_reply(user: &str) -> (String, &'static str, Option<serde_json::Value>) {
    let message = last_learner_message(user);
    let lowered = message.to_lowercase();
    let title = section(user, "Node: ").unwrap_or("当前知识点");
    let objective = section(user, "Objective: ")
        .unwrap_or("")
        .trim_end_matches(|c: char| matches!(c, '。' | '.' | '！' | '？'));
    let diagnostic = section(user, "Diagnostic question: ").unwrap_or("");
    let focus = if !diagnostic.is_empty() {
        diagnostic
    } else if !objective.is_empty() {
        objective
    } else {
        "你会怎样用一个具体例子说明它？"
    };

    if message.is_empty() {
        return (
            format!("我在。关于「{title}」，你可以先用自己的话说说：{focus}"),
            "ask",
            None,
        );
    }
    if lowered.contains("不懂")
        || lowered.contains("不会")
        || lowered.contains("不知道")
        || lowered.contains("没学过")
        || lowered.contains("困惑")
        || lowered.contains("confus")
    {
        return (
            format!(
                "没关系，我们把它拆小一点。先记住「{title}」想解决的是：{objective}。\
                 现在只做一件事：{focus}\n\n你先试着回答这一小步，不用怕答错。"
            ),
            "explain",
            Some(json!({
                "kind": "Misconception",
                "knowledgePointIds": [],
                "confidence": 0.82,
                "severity": 0.7,
                "explanation": "学习者主动表示不理解，可能停留在名词层面。",
            })),
        );
    }
    if lowered.contains("直接讲")
        || lowered.contains("告诉我答案")
        || lowered.contains("讲一下")
        || lowered.contains("解释一下")
    {
        return (
            format!(
                "好，我先给你一个最小解释：{objective}。\
                 但直接记结论容易忘，你把它代回一个具体例子：{focus}"
            ),
            "explain",
            None,
        );
    }
    if lowered.contains("例子") || lowered.contains("举例") {
        return (
            format!(
                "可以。最小例子：先取一个你熟悉的场景，把「{title}」放进去，只改变一个条件，看结论哪里变了。\
                 你愿意先说说你想到的例子吗？"
            ),
            "example",
            None,
        );
    }
    if lowered.contains("为什么") || lowered.contains("原理") || lowered.contains("怎么会") {
        return (
            format!(
                "这个问题问到点上了。先不急着给证明，我们从一个反例开始：如果「{title}」不成立，会发生什么？\
                 你先猜一个可能崩掉的地方。"
            ),
            "hint",
            None,
        );
    }
    if lowered.contains("懂")
        || lowered.contains("明白")
        || lowered.contains("原来")
        || lowered.contains("continue")
    {
        return (
            format!("很好。那换一个稍微不同的情境，你还能用「{title}」解释吗？{focus}"),
            "transition",
            None,
        );
    }

    let snippet: String = message.chars().take(24).collect();
    (
        format!(
            "我听到的是「{snippet}…」。我们先确认一下：你说的这个理解，能对应到「{title}」的哪个具体例子？\
             如果你愿意，也可以直接说哪里不确定。"
        ),
        "probe",
        None,
    )
}

fn tutor_v1_json(user: &str) -> Result<String, AppError> {
    let (reply, action, diagnosis) = tutor_offline_reply(user);
    let citations = citation_labels(user);
    let grounding = if citations.is_empty() {
        "model_knowledge"
    } else {
        "source_grounded"
    };
    Ok(json!({
        "message": reply,
        "citations": citations,
        "suggestedAction": action,
        "grounding": grounding,
        "diagnosis": diagnosis,
    })
    .to_string())
}

fn tutor_stream_text(user: &str) -> String {
    let (body, _action, diagnosis) = tutor_offline_reply(user);
    let citations = citation_labels(user);
    let citation_suffix = if citations.is_empty() {
        String::new()
    } else {
        let labels = citations
            .iter()
            .map(|label| format!("[{label}]"))
            .collect::<Vec<_>>()
            .join(" ");
        format!("（参考本地资料：{labels}）")
    };
    let mut output = if citation_suffix.is_empty() {
        body
    } else {
        format!("{body} {citation_suffix}")
    };
    if let Some(diagnosis) = diagnosis {
        output.push_str(&format!("\\nDIAGNOSIS: {diagnosis}"));
    }
    output
}

fn summary_json(user: &str) -> String {
    let preview: String = user.chars().take(400).collect();
    format!(
        "- 已完成本段对话的离线摘要\n- 学习者仍在围绕当前知识点练习\n- 摘要预览：{}",
        preview.replace('\n', " ")
    )
}

fn quiz_json(user: &str) -> Result<String, AppError> {
    let title = section(user, "Node title: ")
        .or_else(|| section(user, "Node: "))
        .unwrap_or("当前知识点");
    let concept = section(user, "Knowledge point: ").unwrap_or(title);
    Ok(json!({
        "questions": [
            {
                "kind": "single",
                "prompt": format!("关于「{concept}」，下面哪一句最准确？"),
                "options": [
                    format!("{concept} 是本节的核心概念"),
                    "它与本节内容无关",
                    "它只用于记忆，不需要理解"
                ],
                "answers": [format!("{concept} 是本节的核心概念")],
                "explanation": format!("{concept} 是当前学习节点要掌握的内容。"),
                "difficulty": 0.35
            },
            {
                "kind": "fill",
                "prompt": format!("用一个词概括本节的核心：{concept}"),
                "options": [],
                "answers": [concept],
                "explanation": "填写本节的核心概念即可。",
                "difficulty": 0.3
            }
        ]
    })
    .to_string())
}

fn last_learner_message(user: &str) -> &str {
    user.rsplit("\nuser:")
        .next()
        .and_then(|rest| rest.lines().next())
        .map(str::trim)
        .unwrap_or_else(|| user.trim())
}

fn citation_labels(user: &str) -> Vec<String> {
    let Ok(regex) = regex::Regex::new(r"\[C(\d+)\]") else {
        return Vec::new();
    };
    let mut labels: Vec<String> = regex
        .captures_iter(user)
        .filter_map(|captures| captures.get(1))
        .map(|value| format!("C{}", value.as_str()))
        .collect();
    labels.dedup();
    labels.truncate(2);
    labels
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
