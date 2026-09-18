use super::dto::{
    AdaptationEventDto, DiagnosisDto, TutorCitationDto, TutorTurnDto, TutorTurnInput,
};
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
    let citations = turn.citations.iter().map(TutorCitationDto::from).collect();
    let diagnosis_detail = turn.diagnosis_detail.as_ref().map(DiagnosisDto::from);
    Ok(TutorTurnDto {
        message: turn.message,
        suggested_action: turn.suggested_action,
        diagnosis: turn.diagnosis,
        diagnosis_detail,
        citations,
        grounding: turn.grounding,
        generated_by: turn.generated_by,
        route_updated: turn.adaptation.is_some(),
        inserted_node_id: turn.inserted_node_id,
        adaptation: turn.adaptation.as_ref().map(AdaptationEventDto::from),
    })
}
