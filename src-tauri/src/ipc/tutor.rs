use super::dto::{AdaptationEventDto, TutorTurnDto, TutorTurnInput};
use crate::error::AppError;
use crate::services::tutor_service::TutorService;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn tutor_send(
    state: State<'_, AppState>,
    input: TutorTurnInput,
) -> Result<TutorTurnDto, AppError> {
    let turn = TutorService::new(&state)
        .turn(input.session_id, &input.message)
        .await?;
    Ok(TutorTurnDto {
        message: turn.message,
        suggested_action: turn.suggested_action,
        diagnosis: turn.diagnosis,
        generated_by: turn.generated_by,
        route_updated: turn.adaptation.is_some(),
        adaptation: turn.adaptation.as_ref().map(AdaptationEventDto::from),
    })
}
