use crate::ai::prompts::{
    labeled_evidence, tutor_context_messages, LabeledEvidence, SUMMARY_SYSTEM, SUMMARY_V1, TUTOR_V1,
};
use crate::ai::schemas::parse_json;
use crate::ai::traits::{ChatMessage, ChatProvider, ChatRequest};
use crate::db::repo::{
    courses as course_repo, learning as learning_repo, learning_loop as loop_repo,
};
use crate::domain::{Diagnosis, DiagnosisKind, LearningSessionStatus, ProviderConfig};
use crate::error::AppError;
use crate::knowledge::hybrid::{HybridHit, HybridRetriever, RetrievalFilter};
use crate::services::adaptation_service::AdaptationService;
use crate::state::AppState;
use crate::util::time::utc_now;
use regex::Regex;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

const RECENT_MESSAGE_WINDOW: usize = 8;
const SUMMARY_TRIGGER_MESSAGES: usize = 12;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TutorDiagnosisSchema {
    kind: String,
    #[serde(default)]
    knowledge_point_ids: Vec<Uuid>,
    #[serde(default = "half")]
    confidence: f64,
    #[serde(default = "half")]
    severity: f64,
    #[serde(default)]
    explanation: String,
}

fn half() -> f64 {
    0.5
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TutorReply {
    message: String,
    #[serde(default)]
    citations: Vec<String>,
    #[serde(default)]
    suggested_action: String,
    #[serde(default)]
    grounding: Option<String>,
    #[serde(default)]
    diagnosis: Option<TutorDiagnosisSchema>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorCitation {
    pub chunk_id: Uuid,
    pub source_id: Uuid,
    pub source_title: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct TutorTurn {
    pub message: String,
    pub suggested_action: String,
    /// Human-readable diagnosis explanation, kept for the existing wire shape.
    pub diagnosis: Option<String>,
    /// Structured diagnosis for the adaptation pipeline and future UI.
    pub diagnosis_detail: Option<Diagnosis>,
    pub citations: Vec<TutorCitation>,
    pub grounding: String,
    pub generated_by: String,
    pub adaptation: Option<crate::domain::AdaptationEvent>,
    pub inserted_node_id: Option<Uuid>,
}

pub struct TutorService<'a> {
    state: &'a AppState,
}

impl<'a> TutorService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn turn(&self, session_id: Uuid, message: &str) -> Result<TutorTurn, AppError> {
        let message = message.trim();
        if message.is_empty() {
            return Err(AppError::InvalidInput("message cannot be empty".into()));
        }
        let session = learning_repo::get_session(&self.state.pool, session_id).await?;
        if session.status != LearningSessionStatus::Active {
            return Err(AppError::InvalidInput(
                "learning session is not active".into(),
            ));
        }
        let node = course_repo::get_node(&self.state.pool, session.node_id).await?;
        let lesson: Option<crate::ai::schemas::Lesson> = session
            .summary_json
            .as_deref()
            .and_then(|value| parse_json(value).ok());

        loop_repo::insert_message(&self.state.pool, session_id, "user", message, "{}").await?;

        let steps = loop_repo::list_lesson_steps(&self.state.pool, node.id).await?;
        let plan_summary = if steps.is_empty() {
            "（尚未生成学习计划）".to_owned()
        } else {
            steps
                .iter()
                .map(|step| format!("{}. {}", step.ordinal + 1, step.title))
                .collect::<Vec<_>>()
                .join("\n")
        };

        // ---- hybrid retrieval + context builder ---------------------------
        // FTS5 lexical hits and vector hits are merged with reciprocal rank
        // fusion, then scoped to this route so route A cannot cite route B.
        let retrieval_query = format!("{message}\n{}\n{}", node.title, node.objective);
        let retrieved = HybridRetriever::new(self.state)
            .retrieve(
                &retrieval_query,
                RetrievalFilter::for_route(session.course_id).with_top_k(8),
            )
            .await
            .unwrap_or_default();
        let evidence_pairs: Vec<(Uuid, Option<String>, String)> = retrieved
            .iter()
            .map(|hit| {
                (
                    hit.chunk_id,
                    Some(hit.source_title.clone()),
                    hit.text.clone(),
                )
            })
            .collect();
        let labeled = labeled_evidence(&evidence_pairs);
        let label_map = build_label_map(&labeled, &retrieved);

        let recent = loop_repo::list_messages(&self.state.pool, session_id).await?;
        let recent_messages: Vec<ChatMessage> = recent
            .iter()
            .rev()
            .take(RECENT_MESSAGE_WINDOW)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|(role, content)| ChatMessage {
                role: role.clone(),
                content: content.clone(),
            })
            .collect();
        let summary = load_session_summary(&self.state.pool, session_id).await?;

        let learner_context = format!(
            "Node: {}\nObjective: {}\nLesson explanation:\n{}\n\nLearning plan:\n{}\n\nCurrent mastery: {:.2}",
            node.title,
            node.objective,
            lesson
                .as_ref()
                .map(|lesson| lesson.explanation.as_str())
                .unwrap_or(""),
            plan_summary,
            learning_repo::get_mastery(&self.state.pool, node.id)
                .await
                .map(|state| state.mastery)
                .unwrap_or(0.0),
        );

        let (config, chat) = self.state.providers.default_chat_provider().await?;
        let is_mock = crate::ai::registry::is_mock_config(&config);
        let messages = tutor_context_messages(
            &learner_context,
            summary.as_deref(),
            &recent_messages,
            &labeled,
        );
        let response = chat
            .chat(ChatRequest::json(config.chat_model.clone(), messages))
            .await?;
        let reply: TutorReply = parse_json(&response.content).unwrap_or_else(|error| {
            tracing::warn!(%error, "tutor reply was not parseable JSON");
            TutorReply {
                message: response.content.clone(),
                citations: Vec::new(),
                suggested_action: "continue".to_owned(),
                grounding: None,
                diagnosis: None,
            }
        });

        let citations = resolve_citations(&reply, &label_map);
        let explicit_source_request = asks_for_sources(message);
        let grounding = choose_grounding(
            explicit_source_request,
            evidence_pairs.is_empty(),
            &citations,
            reply.grounding.as_deref(),
        );

        let diagnosis_detail = reply.diagnosis.as_ref().map(|diagnosis| {
            let point_ids = if diagnosis.knowledge_point_ids.is_empty() {
                Vec::new()
            } else {
                diagnosis.knowledge_point_ids.clone()
            };
            Diagnosis::new(
                DiagnosisKind::parse(&diagnosis.kind),
                point_ids,
                diagnosis.confidence,
                diagnosis.severity,
                diagnosis.explanation.clone(),
            )
        });

        loop_repo::insert_message(
            &self.state.pool,
            session_id,
            "assistant",
            &reply.message,
            &serde_json::json!({
                "suggestedAction": reply.suggested_action,
                "grounding": grounding,
                "citations": citations.iter().map(|citation| citation.chunk_id).collect::<Vec<_>>(),
                "generatedBy": if is_mock { "mock" } else { "ai" },
            })
            .to_string(),
        )
        .await?;

        let mut adaptation = None;
        let mut inserted_node_id = None;
        if let Some(diagnosis) = diagnosis_detail.as_ref() {
            // A low-confidence diagnosis is deliberately not allowed to
            // schedule or insert anything: the policy returns None.
            let outcome = AdaptationService::new(self.state)
                .apply(session.course_id, node.id, diagnosis)
                .await?;
            adaptation = outcome.event;
            inserted_node_id = outcome.inserted_node_id;
        }

        if !is_mock {
            let _ = record_usage(
                &self.state.pool,
                "tutor",
                &config,
                TUTOR_V1,
                response.usage.as_ref(),
            )
            .await;
        }
        let _ = self
            .refresh_session_summary(session_id, &config, chat.as_ref())
            .await;

        Ok(TutorTurn {
            message: reply.message,
            suggested_action: if reply.suggested_action.trim().is_empty() {
                "continue".to_owned()
            } else {
                reply.suggested_action
            },
            diagnosis: diagnosis_detail
                .as_ref()
                .map(|diagnosis| diagnosis.explanation.clone()),
            diagnosis_detail,
            citations,
            grounding,
            generated_by: if is_mock { "mock".into() } else { "ai".into() },
            adaptation,
            inserted_node_id,
        })
    }

    async fn refresh_session_summary(
        &self,
        session_id: Uuid,
        config: &ProviderConfig,
        chat: &dyn ChatProvider,
    ) -> Result<(), AppError> {
        let messages = loop_repo::list_messages(&self.state.pool, session_id).await?;
        if messages.len() < SUMMARY_TRIGGER_MESSAGES {
            return Ok(());
        }
        let existing_count: Option<i64> =
            sqlx::query_scalar("SELECT message_count FROM session_summaries WHERE session_id = ?")
                .bind(session_id)
                .fetch_optional(&self.state.pool)
                .await?;
        if existing_count.map(|count| count as usize) == Some(messages.len()) {
            return Ok(());
        }
        let transcript = messages
            .iter()
            .map(|(role, content)| format!("{role}: {content}"))
            .collect::<Vec<_>>()
            .join("\n");
        let response = chat
            .chat(ChatRequest::new(
                config.chat_model.clone(),
                vec![
                    ChatMessage::system(SUMMARY_SYSTEM),
                    ChatMessage::user(transcript),
                ],
            ))
            .await?;
        sqlx::query(
            "INSERT INTO session_summaries (session_id, message_count, summary, updated_at) \
             VALUES (?, ?, ?, ?) ON CONFLICT(session_id) DO UPDATE SET \
             message_count = excluded.message_count, summary = excluded.summary, updated_at = excluded.updated_at",
        )
        .bind(session_id)
        .bind(messages.len() as i64)
        .bind(response.content.trim())
        .bind(utc_now().to_rfc3339())
        .execute(&self.state.pool)
        .await?;
        let _ = SUMMARY_V1;
        Ok(())
    }
}

fn build_label_map(
    labeled: &[LabeledEvidence],
    retrieved: &[HybridHit],
) -> std::collections::HashMap<String, TutorCitation> {
    let mut map = std::collections::HashMap::new();
    for (label, hit) in labeled.iter().zip(retrieved.iter()) {
        if label.chunk_id != hit.chunk_id {
            continue;
        }
        map.insert(
            label.label.clone(),
            TutorCitation {
                chunk_id: hit.chunk_id,
                source_id: hit.source_id,
                source_title: if hit.source_title.trim().is_empty() {
                    label
                        .source_title
                        .clone()
                        .unwrap_or_else(|| "local document".to_owned())
                } else {
                    hit.source_title.clone()
                },
                text: hit.text.clone(),
            },
        );
    }
    map
}

fn resolve_citations(
    reply: &TutorReply,
    label_map: &std::collections::HashMap<String, TutorCitation>,
) -> Vec<TutorCitation> {
    let mut labels = reply.citations.clone();
    if labels.is_empty() {
        let Ok(regex) = Regex::new(r"\[C(\d+)\]") else {
            return Vec::new();
        };
        labels = regex
            .captures_iter(&reply.message)
            .filter_map(|captures| captures.get(1))
            .map(|value| format!("C{}", value.as_str()))
            .collect();
    }
    let mut output = Vec::new();
    for label in labels {
        if let Some(citation) = label_map.get(&label) {
            if !output
                .iter()
                .any(|existing: &TutorCitation| existing.chunk_id == citation.chunk_id)
            {
                output.push(citation.clone());
            }
        }
        // Unknown labels such as C99 are silently discarded. The model never
        // gets to invent a database row.
    }
    output
}

fn asks_for_sources(message: &str) -> bool {
    let lower = message.to_lowercase();
    message.contains("根据我的资料")
        || message.contains("资料里")
        || message.contains("这本 PDF")
        || message.contains("这份资料")
        || lower.contains("according to my")
        || lower.contains("in my document")
}

fn choose_grounding(
    explicit_source_request: bool,
    no_evidence: bool,
    citations: &[TutorCitation],
    claimed: Option<&str>,
) -> String {
    if explicit_source_request && (no_evidence || citations.is_empty()) {
        return "insufficient_source".to_owned();
    }
    if !citations.is_empty() {
        return "source_grounded".to_owned();
    }
    match claimed {
        Some("insufficient_source") => "insufficient_source".to_owned(),
        _ => "model_knowledge".to_owned(),
    }
}

async fn load_session_summary(
    pool: &sqlx::SqlitePool,
    session_id: Uuid,
) -> Result<Option<String>, AppError> {
    Ok(
        sqlx::query_scalar("SELECT summary FROM session_summaries WHERE session_id = ?")
            .bind(session_id)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn record_usage(
    pool: &sqlx::SqlitePool,
    operation: &str,
    provider: &ProviderConfig,
    prompt_version: &str,
    usage: Option<&crate::ai::traits::ChatUsage>,
) -> Result<(), AppError> {
    let Some(usage) = usage else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO usage_events (id, operation, provider_id, model, prompt_version, input_tokens, output_tokens, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4())
    .bind(operation)
    .bind(provider.id)
    .bind(&provider.chat_model)
    .bind(prompt_version)
    .bind(usage.prompt_tokens.map(i64::from))
    .bind(usage.completion_tokens.map(i64::from))
    .bind(utc_now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

#[allow(dead_code)]
fn _assert_chat_provider_is_object_safe(_: Arc<dyn ChatProvider>) {}
