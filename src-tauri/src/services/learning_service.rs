use crate::ai::prompts::evaluation_messages;
use crate::ai::schemas::{parse_json, sanitize_evaluation, Evaluation, Lesson};
use crate::ai::traits::ChatRequest;
use crate::db::repo::{courses as course_repo, learning as learning_repo};
use crate::domain::{CourseNodeStatus, LearningAttempt, LearningSession, LearningSessionStatus};
use crate::error::AppError;
use crate::jobs::manager::JobHandle;
use crate::learning::evaluator::next_mastery;
use crate::services::knowledge_service::KnowledgeService;
use crate::state::AppState;
use crate::util::time::utc_now;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LearningStart {
    pub session: LearningSession,
    /// The same prepared payload the lesson screen already showed, so starting a
    /// session never regenerates or contradicts it.
    pub prepared: crate::services::course_service::PreparedLesson,
    pub mastery: f64,
}

#[derive(Debug, Clone)]
pub struct LearningResponse {
    pub evaluation: Evaluation,
    pub mastery: f64,
    pub attempts: i64,
}

pub struct LearningService<'a> {
    state: &'a AppState,
}

impl<'a> LearningService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn start(
        &self,
        node_id: Uuid,
        handle: Option<&JobHandle>,
    ) -> Result<LearningStart, AppError> {
        let node = course_repo::get_node(&self.state.pool, node_id).await?;
        let prepared = crate::services::course_service::CourseService::new(self.state)
            .prepare_node(node_id, handle)
            .await?;
        let now: DateTime<Utc> = utc_now();
        let session = LearningSession {
            id: Uuid::new_v4(),
            course_id: node.course_id,
            node_id,
            status: LearningSessionStatus::Active,
            started_at: now,
            completed_at: None,
            summary_json: Some(
                serde_json::to_string(&prepared.lesson)
                    .map_err(|error| AppError::Internal(error.to_string()))?,
            ),
        };
        learning_repo::insert_session(&self.state.pool, &session).await?;
        course_repo::update_node_status(&self.state.pool, node_id, CourseNodeStatus::InProgress)
            .await?;
        let mastery = prepared.mastery;
        Ok(LearningStart {
            session,
            prepared,
            mastery,
        })
    }

    pub async fn respond(
        &self,
        session_id: Uuid,
        answer: &str,
        handle: Option<&JobHandle>,
    ) -> Result<LearningResponse, AppError> {
        if answer.trim().is_empty() {
            return Err(AppError::InvalidInput("answer cannot be empty".into()));
        }
        let session = learning_repo::get_session(&self.state.pool, session_id).await?;
        if session.status != LearningSessionStatus::Active {
            return Err(AppError::InvalidInput(
                "learning session is not active".into(),
            ));
        }
        let node = course_repo::get_node(&self.state.pool, session.node_id).await?;

        let lesson: Lesson = session
            .summary_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok())
            .or_else(|| {
                node.lesson_cache_json
                    .as_deref()
                    .and_then(|value| serde_json::from_str(value).ok())
            })
            .ok_or_else(|| AppError::NotFound("lesson content for session is missing".into()))?;

        // Evidence sharpens the grade but is not required: a lesson's own
        // explanation is enough to judge a free-text answer, and a brand new
        // route may legitimately have no imported sources yet.
        let knowledge = KnowledgeService::new(self.state);
        let evidence = knowledge
            .evidence_for_node(&node, 12)
            .await
            .unwrap_or_default();
        let snippets = KnowledgeService::evidence_snippets(&evidence);
        let mastery_state = learning_repo::get_mastery(&self.state.pool, node.id).await?;

        if let Some(handle) = handle {
            handle.progress(0.40, "Evaluating answer").await?;
        }
        let (provider_config, chat_provider) = self.state.providers.default_chat_provider().await?;
        let messages = evaluation_messages(
            &node.title,
            &lesson.explanation,
            &lesson.diagnostic_question,
            answer,
            mastery_state.mastery,
            &snippets,
        )?;
        let response = chat_provider
            .chat(ChatRequest::json(
                provider_config.chat_model.clone(),
                messages,
            ))
            .await?;
        let evaluation = sanitize_evaluation(parse_json::<Evaluation>(&response.content)?)?;
        let new_mastery = next_mastery(mastery_state.mastery, &evaluation, answer.chars().count());
        let attempts = mastery_state.attempts + 1;

        let attempt = LearningAttempt {
            id: Uuid::new_v4(),
            session_id,
            node_id: node.id,
            user_answer: answer.to_owned(),
            evaluation_json: serde_json::to_string(&evaluation)
                .map_err(|error| AppError::Internal(error.to_string()))?,
            correctness: evaluation.correctness,
            created_at: utc_now(),
        };
        learning_repo::insert_attempt(&self.state.pool, &attempt).await?;
        learning_repo::upsert_mastery(&self.state.pool, node.id, new_mastery, attempts).await?;
        let status = if new_mastery >= 0.8 {
            CourseNodeStatus::Completed
        } else {
            CourseNodeStatus::InProgress
        };
        course_repo::update_node_status(&self.state.pool, node.id, status).await?;
        if let Some(handle) = handle {
            handle.progress(0.95, "Mastery updated").await?;
        }

        Ok(LearningResponse {
            evaluation,
            mastery: new_mastery,
            attempts,
        })
    }

    pub async fn finish(&self, session_id: Uuid) -> Result<LearningSession, AppError> {
        let session = learning_repo::get_session(&self.state.pool, session_id).await?;
        if session.status != LearningSessionStatus::Active {
            return Ok(session);
        }
        learning_repo::finish_session(
            &self.state.pool,
            session_id,
            &serde_json::json!({ "completed": true }).to_string(),
        )
        .await?;
        learning_repo::get_session(&self.state.pool, session_id).await
    }
}
