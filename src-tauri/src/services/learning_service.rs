use crate::ai::prompts::{free_text_evaluation_messages, labeled_evidence, EVALUATION_V1};
use crate::ai::schemas::{validate_free_text_evaluation, FreeTextEvaluation, Lesson};
use crate::ai::structured::generate_structured;
use crate::db::repo::{
    courses as course_repo, learning as learning_repo, learning_loop as loop_repo,
};
use crate::domain::{
    AdaptationEvent, CourseNodeStatus, Diagnosis, DiagnosisKind, LearningAttempt, LearningSession,
    LearningSessionStatus,
};
use crate::error::AppError;
use crate::jobs::manager::JobHandle;
use crate::learning::evaluator::next_mastery;
use crate::services::adaptation_service::AdaptationService;
use crate::services::knowledge_service::KnowledgeService;
use crate::state::AppState;
use crate::util::time::utc_now;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LearningStart {
    pub session: LearningSession,
    pub prepared: crate::services::course_service::PreparedLesson,
    pub mastery: f64,
}

#[derive(Debug, Clone)]
pub struct LearningResponse {
    pub evaluation: crate::ai::schemas::Evaluation,
    pub mastery: f64,
    pub attempts: i64,
    pub adaptation: Option<AdaptationEvent>,
    pub inserted_node_id: Option<Uuid>,
    pub diagnoses: Vec<Diagnosis>,
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

        let knowledge = KnowledgeService::new(self.state);
        let evidence = knowledge
            .evidence_for_node(&node, 10)
            .await
            .unwrap_or_default();
        let mut evidence_pairs = Vec::new();
        for item in &evidence {
            let source = knowledge.source_for_chunk(&item.chunk).await.ok();
            evidence_pairs.push((
                item.chunk.id,
                source.as_ref().map(|source| source.title.clone()),
                item.chunk.text.clone(),
            ));
        }
        let labeled = labeled_evidence(&evidence_pairs);
        let mastery_state = learning_repo::get_mastery(&self.state.pool, node.id).await?;
        let point_ids: Vec<Uuid> = loop_repo::points_for_node(&self.state.pool, node.id)
            .await?
            .into_iter()
            .map(|point| point.id)
            .collect();
        let expected_concepts = lesson.key_points.clone();
        let knowledge_point = node.title.clone();

        if let Some(handle) = handle {
            handle.progress(0.40, "Evaluating answer").await?;
        }
        let (provider_config, chat_provider) = self.state.providers.default_chat_provider().await?;
        let messages = free_text_evaluation_messages(
            &node.title,
            &lesson.diagnostic_question,
            &expected_concepts,
            &lesson.explanation,
            &knowledge_point,
            answer,
            &labeled,
        );
        let raw = generate_structured::<FreeTextEvaluation, _>(
            chat_provider.as_ref(),
            &provider_config.chat_model,
            messages,
            validate_free_text_evaluation,
        )
        .await?;

        let verdict = raw.verdict.to_ascii_lowercase();
        let correctness = match verdict.as_str() {
            "correct" => 1.0,
            "partial" => 0.6,
            "incorrect" => 0.0,
            _ => 0.5,
        };
        let mut diagnoses = raw.diagnoses.clone();
        if diagnoses.is_empty() && verdict == "incorrect" {
            diagnoses.push(Diagnosis::new(
                DiagnosisKind::IncorrectAnswer,
                point_ids.clone(),
                raw.confidence.max(0.6),
                0.85,
                if raw.gaps.is_empty() {
                    "自由作答没有命中核心概念".to_owned()
                } else {
                    raw.gaps.join("；")
                },
            ));
        } else if diagnoses.is_empty() && verdict == "partial" {
            diagnoses.push(Diagnosis::new(
                DiagnosisKind::PartialUnderstanding,
                point_ids.clone(),
                raw.confidence.max(0.6),
                0.6,
                if raw.gaps.is_empty() {
                    "自由作答方向正确，但缺少关键环节".to_owned()
                } else {
                    raw.gaps.join("；")
                },
            ));
        }
        for diagnosis in &mut diagnoses {
            if diagnosis.knowledge_point_ids.is_empty() {
                diagnosis.knowledge_point_ids = point_ids.clone();
            }
        }

        let evaluation = crate::ai::schemas::Evaluation {
            feedback: raw.feedback.clone(),
            correctness,
            misconceptions: raw.gaps.clone(),
            mastery_delta: raw.mastery_delta,
            follow_up_question: None,
            suggested_action: match verdict.as_str() {
                "correct" => "advance".to_owned(),
                "partial" => "practice".to_owned(),
                _ => "insert_review".to_owned(),
            },
            verdict: Some(raw.verdict.clone()),
            strengths: raw.strengths.clone(),
            gaps: raw.gaps.clone(),
            diagnoses: diagnoses.clone(),
            confidence: Some(raw.confidence),
        };
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

        // ---- unified adaptation ------------------------------------------
        let mut adaptation = None;
        let mut inserted_node_id = None;
        for diagnosis in &diagnoses {
            let outcome = AdaptationService::new(self.state)
                .apply(session.course_id, node.id, diagnosis)
                .await?;
            if outcome.event.is_some() {
                adaptation = outcome.event;
            }
            if outcome.inserted_node_id.is_some() {
                inserted_node_id = outcome.inserted_node_id;
            }
        }
        let _ = crate::services::tutor_service::record_usage(
            &self.state.pool,
            "free_text_evaluation",
            &provider_config,
            EVALUATION_V1,
            None,
        )
        .await;
        if let Some(handle) = handle {
            handle.progress(0.95, "Mastery updated").await?;
        }

        Ok(LearningResponse {
            evaluation,
            mastery: new_mastery,
            attempts,
            adaptation,
            inserted_node_id,
            diagnoses,
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
