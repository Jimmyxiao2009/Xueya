use crate::db::repo::{courses as course_repo, learning_loop as loop_repo};
use crate::domain::{
    AdaptationDecision, AdaptationEvent, AdaptationPolicy, Diagnosis, DiagnosisKind,
};
use crate::error::AppError;
use crate::services::course_service::CourseService;
use crate::state::AppState;
use crate::util::time::utc_now;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AdaptationOutcome {
    pub decision: AdaptationDecision,
    pub event: Option<AdaptationEvent>,
    pub inserted_node_id: Option<Uuid>,
}

/// The only component allowed to change a roadmap in response to learning
/// evidence. Quiz grading, free-text evaluation and tutor diagnosis all call
/// this service.
pub struct AdaptationService<'a> {
    state: &'a AppState,
}

impl<'a> AdaptationService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn apply(
        &self,
        course_id: Uuid,
        after_node_id: Uuid,
        diagnosis: &Diagnosis,
    ) -> Result<AdaptationOutcome, AppError> {
        let decision = AdaptationPolicy.decide(diagnosis);
        match decision {
            AdaptationDecision::None => Ok(AdaptationOutcome {
                decision,
                event: None,
                inserted_node_id: None,
            }),
            AdaptationDecision::InsertReview => {
                self.insert_review(course_id, after_node_id, diagnosis)
                    .await
            }
            AdaptationDecision::IncreasePractice => {
                // IncreasePractice is deliberately handled by the same
                // idempotent insertion path today: the learner sees a targeted
                // practice node, and the policy decision is still recorded.
                self.insert_review(course_id, after_node_id, diagnosis)
                    .await
            }
            AdaptationDecision::ScheduleReview => self.schedule_review(course_id, diagnosis).await,
            AdaptationDecision::AdjustMastery => Ok(AdaptationOutcome {
                decision,
                event: None,
                inserted_node_id: None,
            }),
        }
    }

    async fn insert_review(
        &self,
        course_id: Uuid,
        after_node_id: Uuid,
        diagnosis: &Diagnosis,
    ) -> Result<AdaptationOutcome, AppError> {
        let after = course_repo::get_node(&self.state.pool, after_node_id).await?;
        let point_id = self.point_for_diagnosis(after_node_id, diagnosis).await?;
        let title = crate::domain::adaptation::review_node_title(&after.title);

        if let Some(point_id) = point_id {
            if course_repo::review_node_exists_for_point(
                &self.state.pool,
                course_id,
                point_id,
                after_node_id,
            )
            .await?
            {
                return Ok(AdaptationOutcome {
                    decision: AdaptationDecision::InsertReview,
                    event: None,
                    inserted_node_id: None,
                });
            }
        }

        let reason = if diagnosis.explanation.trim().is_empty() {
            diagnosis.kind.as_str().to_owned()
        } else {
            diagnosis.explanation.clone()
        };
        let objective =
            crate::domain::adaptation::review_node_objective(std::slice::from_ref(&reason));
        let review = CourseService::new(self.state)
            .insert_review_node(
                course_id,
                after_node_id,
                &title,
                &objective,
                &reason,
                point_id,
            )
            .await?;

        let event = loop_repo::list_adaptations(&self.state.pool, course_id, 1)
            .await?
            .into_iter()
            .next();
        Ok(AdaptationOutcome {
            decision: AdaptationDecision::InsertReview,
            event,
            inserted_node_id: Some(review.id),
        })
    }

    async fn schedule_review(
        &self,
        course_id: Uuid,
        diagnosis: &Diagnosis,
    ) -> Result<AdaptationOutcome, AppError> {
        // Low-confidence observations schedule a review instead of changing the
        // roadmap. Persisting the learner state is the real side effect.
        for point_id in &diagnosis.knowledge_point_ids {
            let mut state = loop_repo::get_learner_state(&self.state.pool, course_id, *point_id)
                .await?
                .unwrap_or_else(|| crate::domain::LearnerState::fresh(course_id, *point_id));
            state.next_review = Some(crate::domain::schedule_next_review(
                utc_now(),
                state.mastery,
                state.mistakes.max(1),
            ));
            state.updated_at = utc_now();
            loop_repo::upsert_learner_state(&self.state.pool, &state).await?;
        }
        Ok(AdaptationOutcome {
            decision: AdaptationDecision::ScheduleReview,
            event: None,
            inserted_node_id: None,
        })
    }

    async fn point_for_diagnosis(
        &self,
        after_node_id: Uuid,
        diagnosis: &Diagnosis,
    ) -> Result<Option<Uuid>, AppError> {
        if let Some(point_id) = diagnosis.knowledge_point_ids.first().copied() {
            return Ok(Some(point_id));
        }
        Ok(loop_repo::points_for_node(&self.state.pool, after_node_id)
            .await?
            .first()
            .map(|point| point.id))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn apply_kind(
        &self,
        course_id: Uuid,
        after_node_id: Uuid,
        kind: DiagnosisKind,
        point_ids: Vec<Uuid>,
        confidence: f64,
        severity: f64,
        explanation: impl Into<String>,
    ) -> Result<AdaptationOutcome, AppError> {
        let diagnosis = Diagnosis::new(kind, point_ids, confidence, severity, explanation);
        self.apply(course_id, after_node_id, &diagnosis).await
    }
}
