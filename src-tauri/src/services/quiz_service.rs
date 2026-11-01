use crate::ai::prompts::evaluation_messages;
use crate::ai::schemas::{parse_json, sanitize_evaluation, Evaluation};
use crate::ai::traits::ChatRequest;
use crate::db::repo::{courses as course_repo, learning_loop as loop_repo};
use crate::domain::{
    blend_confidence, blend_mastery, grade_locally, schedule_next_review, AdaptationEvent,
    LearnerState, QuizAttempt, QuizGrade, QuizQuestion,
};
use crate::error::AppError;
use crate::services::knowledge_service::KnowledgeService;
use crate::state::AppState;
use crate::util::time::utc_now;
use uuid::Uuid;

/// Everything that happened as a result of one submitted answer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuizOutcome {
    pub grade: QuizGrade,
    pub node_id: Uuid,
    pub course_id: Uuid,
    pub mastery: f64,
    pub confidence: f64,
    pub next_review: Option<chrono::DateTime<chrono::Utc>>,
    pub adaptation: Option<AdaptationEvent>,
    pub inserted_node_id: Option<Uuid>,
    /// False when the answer came back from the offline stand-in and should be
    /// labelled as a heuristic rather than a grade.
    pub authoritative: bool,
}

pub struct QuizService<'a> {
    state: &'a AppState,
}

impl<'a> QuizService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn questions_for_node(&self, node_id: Uuid) -> Result<Vec<QuizQuestion>, AppError> {
        loop_repo::list_quiz_questions(&self.state.pool, node_id).await
    }

    /// Grade one answer, update the learner model and adapt the roadmap.
    pub async fn submit(
        &self,
        question_id: Uuid,
        answers: Vec<String>,
        session_id: Option<Uuid>,
    ) -> Result<QuizOutcome, AppError> {
        let operation_key = format!(
            "quiz_service:{}",
            crate::util::hashing::sha256_text(
                &serde_json::to_string(&(question_id, session_id, &answers))
                    .map_err(|e| AppError::Internal(e.to_string()))?
            )
        );
        if let Some(saved) = sqlx::query_scalar::<_, String>(
            "SELECT payload_json FROM learning_outcomes WHERE operation_key = ?",
        )
        .bind(&operation_key)
        .fetch_optional(&self.state.pool)
        .await?
        {
            return serde_json::from_str(&saved).map_err(|e| AppError::Internal(e.to_string()));
        }

        let question = loop_repo::get_quiz_question(&self.state.pool, question_id).await?;
        let node = course_repo::get_node(&self.state.pool, question.node_id).await?;
        let course = course_repo::get_course(&self.state.pool, node.course_id).await?;

        // A photograph is a real artefact but not yet a machine-readable answer.
        // Recording it as a grade would corrupt the learner model, so the whole
        // "did they understand it" path is skipped and the UI is told the truth.
        if question.kind() == crate::domain::QuizKind::Image {
            return Ok(QuizOutcome {
                grade: QuizGrade {
                    correct: false,
                    graded: false,
                    score: 0.0,
                    feedback:
                        "照片已保存在本机。识图评阅尚未接入，这一题不会自动判分，也不会影响你的掌握度。"
                            .to_owned(),
                    expected: Vec::new(),
                    misconceptions: Vec::new(),
                },
                node_id: node.id,
                course_id: course.id,
                mastery: 0.0,
                confidence: 0.0,
                next_review: None,
                adaptation: None,
                inserted_node_id: None,
                authoritative: false,
            });
        }

        let (grade, authoritative) = match grade_locally(&question, &answers) {
            Some(grade) => (grade, true),
            None => self.tutor_grade(&node, &question, &answers).await?,
        };

        let mut tx = self.state.pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Some(saved) = sqlx::query_scalar::<_, String>(
            "SELECT payload_json FROM learning_outcomes WHERE operation_key = ?",
        )
        .bind(&operation_key)
        .fetch_optional(&mut *tx)
        .await?
        {
            return serde_json::from_str(&saved).map_err(|e| AppError::Internal(e.to_string()));
        }
        // ---- learner model -------------------------------------------------
        let point_id = question.knowledge_point_id.or_else(|| {
            serde_json::from_str::<serde_json::Value>(&node.metadata_json)
                .ok()
                .and_then(|value| value.get("knowledgePointId").cloned())
                .and_then(|value| serde_json::from_value::<Uuid>(value).ok())
        });
        let now = utc_now();
        let mut learner = match point_id {
            Some(point_id) => loop_repo::get_learner_state_tx(&mut *tx, course.id, point_id)
                .await?
                .unwrap_or_else(|| LearnerState::fresh(course.id, point_id)),
            None => LearnerState::fresh(course.id, Uuid::nil()),
        };

        let weight = match question.kind() {
            crate::domain::QuizKind::Single => 0.25,
            crate::domain::QuizKind::Multi => 0.35,
            crate::domain::QuizKind::Fill => 0.35,
            crate::domain::QuizKind::Short => 0.5,
            crate::domain::QuizKind::Image => 0.45,
        };
        learner.mastery = blend_mastery(learner.mastery, grade.score, weight);
        learner.attempts += 1;
        if !grade.correct {
            learner.mistakes += 1;
        }
        learner.confidence = blend_confidence(learner.confidence, learner.attempts);
        learner.last_seen = Some(now);
        learner.next_review = Some(schedule_next_review(now, learner.mastery, learner.mistakes));
        learner.updated_at = now;
        if point_id.is_some() {
            loop_repo::upsert_learner_state_tx(&mut *tx, &learner).await?;
        }
        course_repo::update_node_confidence_tx(&mut *tx, node.id, learner.mastery).await?;

        loop_repo::insert_quiz_attempt_tx(
            &mut *tx,
            &QuizAttempt {
                id: Uuid::new_v4(),
                question_id: question.id,
                node_id: node.id,
                session_id,
                answer_json: serde_json::to_string(&answers)
                    .map_err(|error| AppError::Internal(error.to_string()))?,
                correct: i64::from(grade.correct),
                score: grade.score,
                feedback: grade.feedback.clone(),
                created_at: now,
            },
        )
        .await?;

        // ---- unified adaptation ------------------------------------------
        // Every roadmap mutation goes through AdaptationService. The quiz path
        // normalises a wrong answer into a Diagnosis; it does not insert nodes
        // directly.
        let mut adaptation = None;
        let mut inserted_node_id = None;
        if !grade.correct {
            let diagnosis = crate::domain::Diagnosis::new(
                crate::domain::DiagnosisKind::IncorrectAnswer,
                point_id.into_iter().collect(),
                0.9,
                (1.0 - grade.score).clamp(0.0, 1.0),
                if grade.misconceptions.is_empty() {
                    format!("「{}」的检查题未通过", node.title)
                } else {
                    grade.misconceptions[0].clone()
                },
            );
            let outcome = crate::services::adaptation_service::AdaptationService::new(self.state)
                .apply_tx(&mut *tx, course.id, node.id, &diagnosis)
                .await?;
            adaptation = outcome.event;
            inserted_node_id = outcome.inserted_node_id;
        }

        let outcome = QuizOutcome {
            grade,
            node_id: node.id,
            course_id: course.id,
            mastery: learner.mastery,
            confidence: learner.confidence,
            next_review: learner.next_review,
            adaptation,
            inserted_node_id,
            authoritative,
        };
        sqlx::query("INSERT INTO learning_outcomes (operation_key, payload_json) VALUES (?, ?)")
            .bind(&operation_key)
            .bind(serde_json::to_string(&outcome).map_err(|e| AppError::Internal(e.to_string()))?)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(outcome)
    }

    /// Subjective widgets are judged by the tutor model. When the offline
    /// stand-in answers, the result is explicitly marked as non-authoritative.
    async fn tutor_grade(
        &self,
        node: &crate::domain::CourseNode,
        question: &QuizQuestion,
        answers: &[String],
    ) -> Result<(QuizGrade, bool), AppError> {
        let answer = answers.join("\n");
        if answer.trim().is_empty() {
            return Ok((
                QuizGrade {
                    correct: false,
                    graded: true,
                    score: 0.0,
                    feedback: "还没有作答。写下你的想法，哪怕只有一句也可以。".to_owned(),
                    expected: Vec::new(),
                    misconceptions: vec!["未作答".to_owned()],
                },
                true,
            ));
        }
        let knowledge = KnowledgeService::new(self.state);
        let evidence = match knowledge.evidence_for_node(node, 8).await {
            Ok(chunks) => chunks,
            Err(error) => {
                tracing::debug!(%error, "no evidence for tutor grading");
                Vec::new()
            }
        };
        let snippets = KnowledgeService::evidence_snippets(&evidence);
        let (config, chat) = self.state.providers.default_chat_provider().await?;
        let is_mock = crate::ai::registry::is_mock_config(&config);
        let messages = evaluation_messages(
            &node.title,
            &question.explanation,
            &question.prompt,
            &answer,
            question.difficulty,
            &snippets,
        )?;
        let response = chat
            .chat(ChatRequest::json(config.chat_model.clone(), messages))
            .await?;
        let evaluation: Evaluation = match parse_json(&response.content) {
            Ok(value) => sanitize_evaluation(value)?,
            Err(error) => {
                tracing::warn!(%error, "tutor evaluation was not parseable");
                Evaluation {
                    feedback: "这次没能拿到可靠的评阅结果。先记下你的答案，稍后再看一遍。"
                        .to_owned(),
                    correctness: 0.5,
                    misconceptions: Vec::new(),
                    mastery_delta: 0.0,
                    follow_up_question: None,
                    suggested_action: "continue".to_owned(),
                    verdict: None,
                    strengths: Vec::new(),
                    gaps: Vec::new(),
                    diagnoses: Vec::new(),
                    confidence: None,
                }
            }
        };
        let score = evaluation.correctness.clamp(0.0, 1.0);
        Ok((
            QuizGrade {
                correct: score >= 0.6,
                graded: !is_mock,
                score,
                feedback: evaluation.feedback,
                expected: Vec::new(),
                misconceptions: evaluation.misconceptions,
            },
            !is_mock,
        ))
    }
}
