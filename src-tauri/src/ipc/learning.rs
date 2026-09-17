use super::dto::{
    LearningEvaluationDto, LearningFinishInput, LearningRespondInput, LearningStartDto,
    LearningStartInput, LessonDto,
};
use crate::db::repo::courses as course_repo;
use crate::error::AppError;
use crate::services::learning_service::LearningService;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn learning_start(
    state: State<'_, AppState>,
    input: LearningStartInput,
) -> Result<LearningStartDto, AppError> {
    let started = LearningService::new(&state)
        .start(input.node_id, None)
        .await?;
    let node = course_repo::get_node(&state.pool, input.node_id).await?;
    let lesson = LessonDto::from_prepared(&started.prepared, &node);
    Ok(LearningStartDto {
        session_id: started.session.id,
        lesson,
        mastery: started.mastery,
    })
}

#[tauri::command]
pub async fn learning_respond(
    state: State<'_, AppState>,
    input: LearningRespondInput,
) -> Result<LearningEvaluationDto, AppError> {
    let response = LearningService::new(&state)
        .respond(input.session_id, &input.answer, None)
        .await?;
    Ok(LearningEvaluationDto {
        feedback: response.evaluation.feedback,
        correctness: response.evaluation.correctness,
        misconceptions: response.evaluation.misconceptions,
        mastery_delta: response.evaluation.mastery_delta,
        follow_up_question: response.evaluation.follow_up_question,
        suggested_action: response.evaluation.suggested_action,
        mastery: response.mastery,
        attempts: response.attempts,
    })
}

#[tauri::command]
pub async fn learning_finish(
    state: State<'_, AppState>,
    input: LearningFinishInput,
) -> Result<(), AppError> {
    LearningService::new(&state)
        .finish(input.session_id)
        .await
        .map(|_| ())
}
