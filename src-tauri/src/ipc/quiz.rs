use super::dto::{AdaptationEventDto, QuizQuestionDto, QuizResultDto, QuizSubmitInput};
use crate::db::repo::learning_loop as loop_repo;
use crate::error::AppError;
use crate::services::quiz_service::QuizService;
use crate::state::AppState;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn quiz_list(
    state: State<'_, AppState>,
    node_id: Uuid,
) -> Result<Vec<QuizQuestionDto>, AppError> {
    let questions = QuizService::new(&state).questions_for_node(node_id).await?;
    Ok(questions.iter().map(QuizQuestionDto::from).collect())
}

/// Grade one answer.
///
/// Objective widgets are graded locally in Rust; subjective ones go to the
/// tutor model and come back flagged `graded: false` when the offline stand-in
/// answered, so the UI can label it honestly.
#[tauri::command]
pub async fn quiz_submit(
    state: State<'_, AppState>,
    input: QuizSubmitInput,
) -> Result<QuizResultDto, AppError> {
    let outcome = QuizService::new(&state)
        .submit(input.question_id, input.answer, input.session_id)
        .await?;
    let question = loop_repo::get_quiz_question(&state.pool, input.question_id).await?;
    Ok(QuizResultDto {
        question_id: input.question_id,
        node_id: question.node_id,
        correct: outcome.grade.correct,
        graded: outcome.grade.graded && outcome.authoritative,
        score: outcome.grade.score,
        feedback: outcome.grade.feedback,
        expected: outcome.grade.expected,
        misconceptions: outcome.grade.misconceptions,
        mastery: outcome.mastery,
        confidence: outcome.confidence,
        next_review: outcome.next_review.map(|value| value.to_rfc3339()),
        adaptation: outcome.adaptation.as_ref().map(AdaptationEventDto::from),
        inserted_node_id: outcome.inserted_node_id,
    })
}
