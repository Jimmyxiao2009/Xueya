use super::dto::{
    route_adapted_payload, AdaptationEventDto, DiagnosisDto, TutorCitationDto, TutorStreamInput,
    TutorTurnDto, TutorTurnInput,
};
use crate::ai::stream::AiStreamEvent;
use crate::error::AppError;
use crate::ipc::{AppHandle, State};
use crate::services::tutor_service::{TutorService, TutorTurn};
use crate::state::AppState;
use serde_json::json;
#[cfg(feature = "desktop")]
use tauri::Emitter;
use tokio::sync::mpsc;

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn tutor_send(
    app: AppHandle,
    state: State<'_, AppState>,
    input: TutorTurnInput,
) -> Result<TutorTurnDto, AppError> {
    let turn = TutorService::new(&state)
        .turn(input.session_id, &input.message)
        .await?;
    if let Some(event) = &turn.adaptation {
        let _ = app.emit(
            "route_adapted",
            route_adapted_payload(event, turn.inserted_node_id),
        );
    }
    Ok(tutor_turn_dto(turn))
}

/// Streaming tutor turn.
///
/// Events are emitted on the `tutor_stream` Tauri event as
/// `{ requestId, event }`; the command returns the authoritative final turn.
/// The frontend can buffer deltas with requestAnimationFrame and still apply
/// the same citation/adaptation metadata as the non-streaming path.
#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn tutor_send_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    input: TutorStreamInput,
) -> Result<TutorTurnDto, AppError> {
    let request_id = input
        .request_id
        .clone()
        .unwrap_or_else(|| "tutor-stream".to_owned());
    let (sender, mut receiver) = mpsc::unbounded_channel::<AiStreamEvent>();
    let app_for_forwarder = app.clone();
    let forwarder = tokio::spawn(async move {
        while let Some(event) = receiver.recv().await {
            let _ = app_for_forwarder.emit(
                "tutor_stream",
                json!({ "requestId": request_id, "event": event }),
            );
        }
    });

    let result = TutorService::new(&state)
        .turn_stream(input.session_id, &input.message, sender)
        .await;
    let _ = forwarder.await;
    let turn = result?;
    if let Some(event) = &turn.adaptation {
        let _ = app.emit(
            "route_adapted",
            route_adapted_payload(event, turn.inserted_node_id),
        );
    }
    Ok(tutor_turn_dto(turn))
}

pub(crate) fn tutor_turn_dto(turn: TutorTurn) -> TutorTurnDto {
    let citations = turn.citations.iter().map(TutorCitationDto::from).collect();
    let diagnosis_detail = turn.diagnosis_detail.as_ref().map(DiagnosisDto::from);
    TutorTurnDto {
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
        mastery_delta: turn.mastery_delta,
        mastery: turn.mastery,
        ready_to_complete: turn.ready_to_complete,
    }
}
