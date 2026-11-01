use super::dto::{
    AdaptationEventDto, ConversationMessageDto, DiagnosisDto, LearningEvaluationDto,
    LearningFinishInput, LearningRespondInput, LearningStartDto, LearningStartInput, LessonDto,
};
use crate::db::repo::courses as course_repo;
use crate::error::AppError;
use crate::ipc::tutor::tutor_turn_dto;
use crate::ipc::State;
use crate::services::learning_service::LearningService;
use crate::services::tutor_service::TutorService;
use crate::state::AppState;

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn learning_start(
    state: State<'_, AppState>,
    input: LearningStartInput,
) -> Result<LearningStartDto, AppError> {
    let started = LearningService::new(&state)
        .start(input.node_id, None)
        .await?;
    let node = course_repo::get_node(&state.pool, input.node_id).await?;
    let lesson = LessonDto::from_prepared(&started.prepared, &node);
    let messages = started
        .messages
        .iter()
        .map(|message| ConversationMessageDto {
            id: message.id,
            role: message.role.clone(),
            content: message.content.clone(),
            metadata_json: message.metadata_json.clone(),
            created_at: message.created_at.to_rfc3339(),
        })
        .collect::<Vec<_>>();

    // Opening the session must never depend on the network or on an AI account.
    // A failure is returned as data so the local conversation can start with a
    // clearly-labelled local opening. On resume the persisted opening is already
    // part of `messages`; do not generate a second one.
    let (opening_turn, opening_error) = if started.resumed {
        (None, None)
    } else {
        match TutorService::new(&state).opening(started.session.id).await {
            Ok(turn) => (Some(tutor_turn_dto(turn)), None),
            Err(error) => {
                tracing::warn!(%error, "learning opening turn could not be generated");
                (None, Some(error.to_string()))
            }
        }
    };

    Ok(LearningStartDto {
        session_id: started.session.id,
        lesson,
        mastery: started.mastery,
        opening_turn,
        opening_error,
        resumed: started.resumed,
        messages,
    })
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn learning_respond(
    state: State<'_, AppState>,
    input: LearningRespondInput,
) -> Result<LearningEvaluationDto, AppError> {
    let response = LearningService::new(&state)
        .respond(input.session_id, &input.answer, None)
        .await?;
    let diagnoses: Vec<DiagnosisDto> = response.diagnoses.iter().map(DiagnosisDto::from).collect();
    Ok(LearningEvaluationDto {
        feedback: response.evaluation.feedback,
        correctness: response.evaluation.correctness,
        misconceptions: response.evaluation.misconceptions,
        mastery_delta: response.evaluation.mastery_delta,
        follow_up_question: response.evaluation.follow_up_question,
        suggested_action: response.evaluation.suggested_action,
        mastery: response.mastery,
        attempts: response.attempts,
        verdict: response.evaluation.verdict,
        strengths: response.evaluation.strengths,
        gaps: response.evaluation.gaps,
        diagnoses,
        confidence: response.evaluation.confidence,
        adaptation: response.adaptation.as_ref().map(AdaptationEventDto::from),
        inserted_node_id: response.inserted_node_id,
    })
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn learning_finish(
    state: State<'_, AppState>,
    input: LearningFinishInput,
) -> Result<(), AppError> {
    LearningService::new(&state)
        .finish(input.session_id)
        .await
        .map(|_| ())
}
