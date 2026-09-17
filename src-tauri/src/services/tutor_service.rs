use crate::ai::prompts::tutor_messages;
use crate::ai::schemas::parse_json;
use crate::ai::traits::ChatRequest;
use crate::db::repo::{
    courses as course_repo, learning as learning_repo, learning_loop as loop_repo,
};
use crate::domain::{AdaptationEvent, AdaptationKind, LearningSessionStatus};
use crate::error::AppError;
use crate::services::knowledge_service::KnowledgeService;
use crate::state::AppState;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TutorReply {
    message: String,
    #[serde(default)]
    suggested_action: String,
    #[serde(default)]
    diagnosis: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TutorTurn {
    pub message: String,
    pub suggested_action: String,
    pub diagnosis: Option<String>,
    pub generated_by: String,
    pub adaptation: Option<AdaptationEvent>,
}

pub struct TutorService<'a> {
    state: &'a AppState,
}

impl<'a> TutorService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    /// One turn of the in-lesson tutor.
    ///
    /// The reply *rules* live here, not in the UI: the frontend sends raw text
    /// and renders whatever comes back.
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

        let knowledge = KnowledgeService::new(self.state);
        let evidence = match knowledge.evidence_for_node(&node, 6).await {
            Ok(chunks) => chunks,
            Err(error) => {
                tracing::debug!(%error, "tutor has no evidence for this node");
                Vec::new()
            }
        };
        let snippets = KnowledgeService::evidence_snippets(&evidence);

        let (config, chat) = self.state.providers.default_chat_provider().await?;
        let is_mock = crate::ai::registry::is_mock_config(&config);
        let messages = tutor_messages(
            &node.title,
            lesson
                .as_ref()
                .map(|lesson| lesson.explanation.as_str())
                .unwrap_or(""),
            &plan_summary,
            message,
            &snippets,
        )?;
        let response = chat
            .chat(ChatRequest::json(config.chat_model.clone(), messages))
            .await?;
        let reply: TutorReply = parse_json(&response.content).unwrap_or_else(|error| {
            tracing::warn!(%error, "tutor reply was not parseable JSON");
            TutorReply {
                message: response.content.clone(),
                suggested_action: "continue".to_owned(),
                diagnosis: None,
            }
        });

        loop_repo::insert_message(
            &self.state.pool,
            session_id,
            "assistant",
            &reply.message,
            &serde_json::json!({
                "suggestedAction": reply.suggested_action,
                "generatedBy": if is_mock { "mock" } else { "ai" },
            })
            .to_string(),
        )
        .await?;

        // A named misconception is a real observation: persist it so the
        // roadmap can act on it later.
        let mut adaptation = None;
        if let Some(diagnosis) = reply
            .diagnosis
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            let event = AdaptationEvent::new(
                session.course_id,
                Some(node.id),
                AdaptationKind::MisconceptionDetected,
                diagnosis.to_owned(),
                serde_json::json!({ "source": "tutor", "sessionId": session_id }),
            );
            loop_repo::insert_adaptation(&self.state.pool, &event).await?;
            adaptation = Some(event);
        }

        Ok(TutorTurn {
            message: reply.message,
            suggested_action: if reply.suggested_action.trim().is_empty() {
                "continue".to_owned()
            } else {
                reply.suggested_action
            },
            diagnosis: reply.diagnosis,
            generated_by: if is_mock { "mock".into() } else { "ai".into() },
            adaptation,
        })
    }
}
