use crate::ai::prompts::{
    labeled_evidence, tutor_context_messages, tutor_stream_context_messages, LabeledEvidence,
    SUMMARY_SYSTEM, SUMMARY_V1, TUTOR_STREAM_V1, TUTOR_V1,
};
use crate::ai::prompts::{tutor_opening_messages, TUTOR_OPENING_V1};

use crate::ai::schemas::parse_json;
use crate::ai::stream::AiStreamEvent;
use crate::ai::traits::{ChatMessage, ChatProvider, ChatRequest};
use crate::db::repo::{
    courses as course_repo, learning as learning_repo, learning_loop as loop_repo,
};
use crate::domain::{
    AgentAction, AgentActionKind, Diagnosis, DiagnosisKind, LearnerResponse, LearningSessionStatus,
    LearningTurn, ProviderConfig,
};
use crate::error::AppError;
use crate::knowledge::hybrid::{HybridHit, HybridRetriever, RetrievalFilter};
use crate::services::adaptation_service::AdaptationService;
use crate::state::AppState;
use crate::util::time::utc_now;
use futures_util::StreamExt;
use regex::Regex;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

const RECENT_MESSAGE_WINDOW: usize = 8;
const SUMMARY_TRIGGER_MESSAGES: usize = 12;
const SUMMARY_INTERVAL_MESSAGES: usize = 6;

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
    /// Internal mastery evidence for this conversational turn. Never rendered
    /// as a score in the conversation UI.
    #[serde(default)]
    mastery_delta: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorCitation {
    pub chunk_id: Uuid,
    pub source_id: Uuid,
    pub source_title: String,
    pub source_kind: String,
    pub source_uri: Option<String>,
    pub source_filename: Option<String>,
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
    /// Internal conversational evidence delta for this turn; never rendered as a score.
    pub mastery_delta: f64,
    /// Updated node mastery after applying this turn's evidence.
    pub mastery: f64,
    /// True once the conversation has enough reliable evidence to finish the node.
    pub ready_to_complete: bool,
}

pub struct TutorService<'a> {
    state: &'a AppState,
}

impl<'a> TutorService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }
    /// Start a Socratic session with an agent-authored first turn.
    ///
    /// The opening is persisted as the first assistant message, so reloading a
    /// session and resuming it later preserves the same teaching thread.
    pub async fn opening(&self, session_id: Uuid) -> Result<TutorTurn, AppError> {
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

        let retrieval_query = format!("{}\n{}", node.title, node.objective);
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

        let mastery = learning_repo::get_mastery(&self.state.pool, node.id)
            .await
            .map(|state| state.mastery)
            .unwrap_or(0.0);
        let key_points = lesson
            .as_ref()
            .map(|lesson| lesson.key_points.join("；"))
            .unwrap_or_default();
        let diagnostic_question = lesson
            .as_ref()
            .map(|lesson| lesson.diagnostic_question.clone())
            .unwrap_or_default();
        let learner_context = format!(
            "Node: {}\nObjective: {}\nLesson explanation:\n{}\n\nLearning plan:\n{}\n\nKey points: {}\nDiagnostic question: {}\nCurrent mastery: {:.2}",
            node.title,
            node.objective,
            lesson
                .as_ref()
                .map(|lesson| lesson.explanation.as_str())
                .unwrap_or(""),
            plan_summary,
            key_points,
            diagnostic_question,
            mastery,
        );

        let (config, chat) = self.state.providers.default_chat_provider().await?;
        let is_mock = crate::ai::registry::is_mock_config(&config);
        let messages = tutor_opening_messages(&learner_context, &labeled);
        let response = chat
            .chat(ChatRequest::json(config.chat_model.clone(), messages))
            .await?;
        let reply: TutorReply = parse_json(&response.content).unwrap_or_else(|error| {
            tracing::warn!(%error, "opening tutor reply was not parseable JSON");
            TutorReply {
                message: response.content.clone(),
                citations: Vec::new(),
                suggested_action: "ask".to_owned(),
                grounding: None,
                diagnosis: None,
                mastery_delta: 0.0,
            }
        });
        let citations = resolve_citations(&reply, &label_map);
        let grounding = choose_grounding(
            false,
            evidence_pairs.is_empty(),
            &citations,
            reply.grounding.as_deref(),
        );
        let generated_by =
            crate::ai::provenance::generation_label(&config, !evidence_pairs.is_empty());

        loop_repo::insert_message(
            &self.state.pool,
            session_id,
            "assistant",
            &reply.message,
            &serde_json::json!({
                "suggestedAction": reply.suggested_action,
                "grounding": grounding,
                "citations": citations.iter().map(|citation| citation.chunk_id).collect::<Vec<_>>(),
                "generatedBy": generated_by,
                "opening": true,
            })
            .to_string(),
        )
        .await?;

        if !is_mock {
            let _ = record_usage(
                &self.state.pool,
                "tutor_opening",
                &config,
                TUTOR_OPENING_V1,
                response.usage.as_ref(),
            )
            .await;
        }

        Ok(TutorTurn {
            message: reply.message,
            suggested_action: if reply.suggested_action.trim().is_empty() {
                "ask".to_owned()
            } else {
                reply.suggested_action
            },
            diagnosis: None,
            diagnosis_detail: None,
            citations,
            grounding,
            generated_by,
            adaptation: None,
            inserted_node_id: None,
            mastery_delta: 0.0,
            mastery,
            ready_to_complete: false,
        })
    }

    /// Assemble persisted messages into first-class conversational turns.
    ///
    /// A turn is one agent action plus the learner response that preceded it.
    /// The opening turn has no learner response yet; subsequent turns preserve
    /// both sides so session history remains a teaching conversation rather
    /// than an undifferentiated message log.
    pub async fn turns(&self, session_id: Uuid) -> Result<Vec<LearningTurn>, AppError> {
        let records = loop_repo::list_message_records(&self.state.pool, session_id).await?;
        let mut pending_learner: Option<LearnerResponse> = None;
        let mut turns = Vec::new();

        for record in records {
            match record.role.as_str() {
                "user" => {
                    pending_learner = Some(LearnerResponse {
                        text: record.content,
                        inferred_state: None,
                    });
                }
                "assistant" => {
                    let suggested_action = serde_json::from_str::<serde_json::Value>(
                        &record.metadata_json,
                    )
                    .ok()
                    .and_then(|value| {
                        value
                            .get("suggestedAction")
                            .and_then(|action| action.as_str())
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| "explain".to_owned());
                    turns.push(LearningTurn {
                        id: record.id,
                        session_id: record.session_id,
                        agent_action: AgentAction {
                            kind: AgentActionKind::parse(&suggested_action),
                            suggested_action,
                        },
                        learner_response: pending_learner.take(),
                        created_at: record.created_at,
                    });
                }
                _ => {}
            }
        }

        Ok(turns)
    }




    /// Persist a small amount of mastery evidence from a conversational turn.
    ///
    /// The model can supply `masteryDelta`; when it does not, a confident
    /// diagnosis still moves the internal estimate. The value is never shown
    /// as a score, but it keeps the learner model alive during conversation.
    async fn record_conversational_evidence(
        &self,
        session_id: Uuid,
        node_id: Uuid,
        user_message: &str,
        reply: &TutorReply,
        diagnosis: Option<&Diagnosis>,
    ) -> Result<(f64, f64, i64), AppError> {
        let delta = if reply.mastery_delta.abs() > f64::EPSILON {
            reply.mastery_delta
        } else if let Some(diagnosis) = diagnosis {
            match diagnosis.kind {
                DiagnosisKind::Mastered => 0.25,
                DiagnosisKind::PartialUnderstanding => 0.10,
                DiagnosisKind::IncorrectAnswer | DiagnosisKind::Misconception => -0.06,
                _ => 0.0,
            }
        } else {
            0.0
        };
        let state = learning_repo::get_mastery(&self.state.pool, node_id).await?;
        let mastery = if delta.abs() <= f64::EPSILON {
            state.mastery
        } else {
            let confidence = diagnosis.map(|diagnosis| diagnosis.confidence).unwrap_or(0.5);
            let mastery =
                crate::learning::mastery::update_mastery(state.mastery, delta, confidence);
            let correctness = diagnosis
                .map(|diagnosis| match diagnosis.kind {
                    DiagnosisKind::Mastered => 0.9,
                    DiagnosisKind::PartialUnderstanding => 0.6,
                    DiagnosisKind::IncorrectAnswer => 0.25,
                    DiagnosisKind::Misconception => 0.2,
                    DiagnosisKind::RepeatedFailure => 0.15,
                    DiagnosisKind::LowConfidence | DiagnosisKind::Uncertain => 0.4,
                })
                .unwrap_or_else(|| (0.5 + delta * 2.0).clamp(0.0, 1.0));
            let attempt = crate::domain::LearningAttempt {
                id: Uuid::new_v4(),
                session_id,
                node_id,
                user_answer: user_message.to_owned(),
                evaluation_json: serde_json::json!({
                    "source": "conversation",
                    "masteryDelta": delta,
                    "diagnosis": diagnosis.map(|diagnosis| diagnosis.kind.as_str()),
                })
                .to_string(),
                correctness,
                created_at: utc_now(),
            };
            learning_repo::insert_attempt(&self.state.pool, &attempt).await?;
            learning_repo::upsert_mastery(&self.state.pool, node_id, mastery, state.attempts + 1)
                .await?;
            mastery
        };
        let evidence_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM learning_attempts WHERE session_id = ? AND node_id = ? AND evaluation_json LIKE '%\"source\":\"conversation\"%'",
        )
        .bind(session_id)
        .bind(node_id)
        .fetch_one(&self.state.pool)
        .await?;
        Ok((delta, mastery, evidence_count))
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
                mastery_delta: 0.0,
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
                "generatedBy": crate::ai::provenance::generation_label(&config, !evidence_pairs.is_empty()),
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

        let (mastery_delta, mastery, evidence_count) = match self
            .record_conversational_evidence(
                session_id,
                node.id,
                message,
                &reply,
                diagnosis_detail.as_ref(),
            )
            .await
        {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%error, "conversational mastery evidence was not persisted");
                let mastery = learning_repo::get_mastery(&self.state.pool, node.id)
                    .await
                    .map(|state| state.mastery)
                    .unwrap_or(0.0);
                (reply.mastery_delta, mastery, 0)
            }
        };
        let ready_to_complete =
            ready_to_complete(mastery, evidence_count, &reply, diagnosis_detail.as_ref());


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
            generated_by: crate::ai::provenance::generation_label(&config, !evidence_pairs.is_empty()),
            adaptation,
            inserted_node_id,
            mastery_delta,
            mastery,
            ready_to_complete,
        })
    }

    /// Real streaming tutor turn.
    ///
    /// The provider stream is consumed incrementally and every delta is sent on
    /// `events`; after the stream ends the same citation/diagnosis/adaptation
    /// pipeline as `turn` runs before the terminal metadata events are sent.
    pub async fn turn_stream(
        &self,
        session_id: Uuid,
        message: &str,
        events: UnboundedSender<AiStreamEvent>,
    ) -> Result<TutorTurn, AppError> {
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
        let mastery = learning_repo::get_mastery(&self.state.pool, node.id)
            .await
            .map(|state| state.mastery)
            .unwrap_or(0.0);
        let learner_context = format!(
            "Node: {}\nObjective: {}\nLesson explanation:\n{}\n\nLearning plan:\n{}\n\nCurrent mastery: {:.2}",
            node.title,
            node.objective,
            lesson
                .as_ref()
                .map(|lesson| lesson.explanation.as_str())
                .unwrap_or(""),
            plan_summary,
            mastery,
        );

        let (config, chat) = self.state.providers.default_chat_provider().await?;
        let messages = tutor_stream_context_messages(
            &learner_context,
            summary.as_deref(),
            &recent_messages,
            &labeled,
        );

        let _ = events.send(AiStreamEvent::Started {
            model: config.chat_model.clone(),
        });
        let mut stream = match chat
            .chat_stream(ChatRequest::new(config.chat_model.clone(), messages))
            .await
        {
            Ok(stream) => stream,
            Err(error) => {
                let _ = events.send(AiStreamEvent::Failed {
                    code: error.code().to_owned(),
                    message: error.public_message().to_owned(),
                });
                return Err(error);
            }
        };

        let mut full_text = String::new();
        while let Some(item) = stream.next().await {
            match item {
                Ok(delta) => {
                    if !delta.content.is_empty() {
                        full_text.push_str(&delta.content);
                        let _ = events.send(AiStreamEvent::Delta {
                            text: delta.content,
                        });
                    }
                }
                Err(error) => {
                    let _ = events.send(AiStreamEvent::Failed {
                        code: error.code().to_owned(),
                        message: error.public_message().to_owned(),
                    });
                    return Err(error);
                }
            }
        }

        let (message_text, diagnosis) = parse_stream_tutor_reply(&full_text);
        let diagnosis_detail = diagnosis.as_ref().map(|diagnosis| {
            Diagnosis::new(
                DiagnosisKind::parse(&diagnosis.kind),
                diagnosis.knowledge_point_ids.clone(),
                diagnosis.confidence,
                diagnosis.severity,
                diagnosis.explanation.clone(),
            )
        });
        let suggested_action = stream_suggested_action(diagnosis_detail.as_ref());
        let reply = TutorReply {
            message: message_text,
            citations: Vec::new(),
            suggested_action: suggested_action.clone(),
            grounding: None,
            diagnosis,
            mastery_delta: 0.0,
        };

        let citations = resolve_citations(&reply, &label_map);
        let explicit_source_request = asks_for_sources(message);
        let grounding = choose_grounding(
            explicit_source_request,
            evidence_pairs.is_empty(),
            &citations,
            reply.grounding.as_deref(),
        );

        loop_repo::insert_message(
            &self.state.pool,
            session_id,
            "assistant",
            &reply.message,
            &serde_json::json!({
                "suggestedAction": reply.suggested_action,
                "grounding": grounding,
                "citations": citations.iter().map(|citation| citation.chunk_id).collect::<Vec<_>>(),
                "generatedBy": crate::ai::provenance::generation_label(&config, !evidence_pairs.is_empty()),
                "streamed": true,
            })
            .to_string(),
        )
        .await?;

        let mut adaptation = None;
        let mut inserted_node_id = None;
        if let Some(diagnosis) = diagnosis_detail.as_ref() {
            let outcome = AdaptationService::new(self.state)
                .apply(session.course_id, node.id, diagnosis)
                .await?;
            adaptation = outcome.event;
            inserted_node_id = outcome.inserted_node_id;
        }

        let (mastery_delta, mastery, evidence_count) = match self
            .record_conversational_evidence(
                session_id,
                node.id,
                message,
                &reply,
                diagnosis_detail.as_ref(),
            )
            .await
        {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%error, "conversational mastery evidence was not persisted");
                let mastery = learning_repo::get_mastery(&self.state.pool, node.id)
                    .await
                    .map(|state| state.mastery)
                    .unwrap_or(0.0);
                (reply.mastery_delta, mastery, 0)
            }
        };
        let ready_to_complete =
            ready_to_complete(mastery, evidence_count, &reply, diagnosis_detail.as_ref());


        let _ = self
            .refresh_session_summary(session_id, &config, chat.as_ref())
            .await;

        let turn = TutorTurn {
            message: reply.message,
            suggested_action: suggested_action.clone(),
            diagnosis: diagnosis_detail
                .as_ref()
                .map(|diagnosis| diagnosis.explanation.clone()),
            diagnosis_detail,
            citations,
            grounding,
            generated_by: crate::ai::provenance::generation_label(&config, !evidence_pairs.is_empty()),
            adaptation,
            inserted_node_id,
            mastery_delta,
            mastery,
            ready_to_complete,
        };

        let citation_titles: Vec<String> = turn
            .citations
            .iter()
            .map(|citation| citation.source_title.clone())
            .collect();
        let _ = events.send(AiStreamEvent::CitationMetadata {
            citations: citation_titles,
        });
        let _ = events.send(AiStreamEvent::Completed {
            finish_reason: Some("stop".to_owned()),
        });
        let _ = TUTOR_STREAM_V1;
        Ok(turn)
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
        let previous = sqlx::query_as::<_, (i64, String)>(
            "SELECT message_count, summary FROM session_summaries WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_optional(&self.state.pool)
        .await?;
        let should_refresh = match previous.as_ref() {
            None => true,
            Some((count, _)) => {
                messages.len() >= (*count as usize).saturating_add(SUMMARY_INTERVAL_MESSAGES)
            }
        };
        if !should_refresh {
            return Ok(());
        }
        let start = previous
            .as_ref()
            .map(|(count, _)| (*count as usize).min(messages.len()))
            .unwrap_or(0);
        let new_turns = messages[start..]
            .iter()
            .map(|(role, content)| format!("{role}: {content}"))
            .collect::<Vec<_>>()
            .join("\n");
        let transcript = match previous.as_ref() {
            Some((_, summary)) => {
                format!("Previous summary:\n{summary}\n\nNew turns:\n{new_turns}")
            }
            None => new_turns,
        };
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

fn stream_suggested_action(diagnosis: Option<&Diagnosis>) -> String {
    match diagnosis.map(|diagnosis| diagnosis.kind) {
        Some(DiagnosisKind::Mastered) => "summarize",
        Some(DiagnosisKind::PartialUnderstanding) => "probe",
        Some(DiagnosisKind::Misconception) => "counterexample",
        Some(DiagnosisKind::IncorrectAnswer) => "hint",
        Some(DiagnosisKind::RepeatedFailure) => "explain",
        Some(DiagnosisKind::LowConfidence) | Some(DiagnosisKind::Uncertain) => "explain",
        None => "continue",
    }
    .to_owned()
}


fn ready_to_complete(
    mastery: f64,
    evidence_count: i64,
    reply: &TutorReply,
    diagnosis: Option<&Diagnosis>,
) -> bool {
    let action = AgentActionKind::parse(&reply.suggested_action);
    let natural_finish = matches!(
        action,
        AgentActionKind::Summarize
            | AgentActionKind::Transition
            | AgentActionKind::CompleteConcept
    ) || diagnosis.is_some_and(|diagnosis| diagnosis.kind == DiagnosisKind::Mastered);
    (mastery >= 0.55 && evidence_count >= 4)
        || (mastery >= 0.45 && evidence_count >= 3 && natural_finish)
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
                source_kind: hit.source_kind.clone(),
                source_uri: hit.source_uri.clone(),
                source_filename: hit.source_filename.clone(),
                text: hit.text.clone(),
            },
        );
    }
    map
}

fn parse_stream_tutor_reply(text: &str) -> (String, Option<TutorDiagnosisSchema>) {
    let mut visible_lines = Vec::new();
    let mut diagnosis = None;
    for line in text.lines() {
        let trimmed = line.trim();
        let upper = trimmed.to_ascii_uppercase();
        if let Some(index) = upper.find("DIAGNOSIS:") {
            let payload = trimmed[index + "DIAGNOSIS:".len()..].trim();
            if let Ok(parsed) = serde_json::from_str::<TutorDiagnosisSchema>(payload) {
                diagnosis = Some(parsed);
                continue;
            }
        }
        visible_lines.push(line);
    }
    (visible_lines.join("\n").trim().to_owned(), diagnosis)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn citation(title: &str, text: &str) -> TutorCitation {
        TutorCitation {
            chunk_id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            source_title: title.to_owned(),
            source_kind: "file".to_owned(),
            source_uri: None,
            source_filename: Some("fixture.md".to_owned()),
            text: text.to_owned(),
        }
    }

    fn reply(message: &str, citations: Vec<&str>) -> TutorReply {
        TutorReply {
            message: message.to_owned(),
            citations: citations.into_iter().map(str::to_owned).collect(),
            suggested_action: "continue".to_owned(),
            grounding: None,
            diagnosis: None,
            mastery_delta: 0.0,
        }
    }

    #[test]
    fn resolve_citations_drops_unknown_labels_and_deduplicates() {
        let first = citation("射影平面笔记", "无穷远点是方向的抽象。");
        let second = citation("齐次坐标", "第三个坐标为 0 表示无穷远点。");
        let mut map = HashMap::new();
        map.insert("C1".to_owned(), first.clone());
        map.insert("C2".to_owned(), second.clone());

        let resolved = resolve_citations(
            &reply(
                "答案来自 [C1]、[C99] 和 [C1]",
                vec!["C1", "C99", "C1", "C2"],
            ),
            &map,
        );

        assert_eq!(resolved.len(), 2);
        assert!(resolved.iter().any(|item| item.chunk_id == first.chunk_id));
        assert!(resolved.iter().any(|item| item.chunk_id == second.chunk_id));
        assert!(
            !resolved
                .iter()
                .any(|item| item.source_title.contains("invented")),
            "unknown labels must never create citations"
        );
    }

    #[test]
    fn resolve_citations_reads_inline_labels_when_json_citations_are_empty() {
        let known = citation("射影平面笔记", "平行线共享无穷远点。");
        let mut map = HashMap::new();
        map.insert("C1".to_owned(), known.clone());

        let resolved = resolve_citations(&reply("见 [C1]", Vec::new()), &map);

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].chunk_id, known.chunk_id);
    }

    #[test]
    fn choose_grounding_never_fakes_a_source() {
        let known = citation("射影平面笔记", "无穷远点。");
        assert_eq!(
            choose_grounding(false, false, &[known], Some("model_knowledge")),
            "source_grounded"
        );
        assert_eq!(
            choose_grounding(true, true, &[], None),
            "insufficient_source"
        );
        assert_eq!(choose_grounding(false, false, &[], None), "model_knowledge");
    }

    fn tutor_reply(action: &str) -> TutorReply {
        TutorReply {
            message: String::new(),
            citations: Vec::new(),
            suggested_action: action.to_owned(),
            grounding: None,
            diagnosis: None,
            mastery_delta: 0.0,
        }
    }

    #[test]
    fn conversation_completion_requires_repeated_evidence() {
        assert!(!ready_to_complete(
            0.6,
            1,
            &tutor_reply("completeConcept"),
            None
        ));
        assert!(ready_to_complete(0.6, 4, &tutor_reply("explain"), None));
        assert!(ready_to_complete(0.5, 3, &tutor_reply("summarize"), None));
        assert!(!ready_to_complete(0.4, 3, &tutor_reply("summarize"), None));
    }

}
