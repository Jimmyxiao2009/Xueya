use super::dto::JobDto;
use crate::error::AppError;
use crate::jobs::event::JobEvent;
use crate::state::AppState;
use tauri::ipc::Channel;
use tauri::State;
use tokio::sync::broadcast;
use uuid::Uuid;

#[tauri::command]
pub async fn job_list(state: State<'_, AppState>) -> Result<Vec<JobDto>, AppError> {
    let jobs = state.jobs.list().await?;
    Ok(jobs.iter().map(JobDto::from).collect())
}

#[tauri::command]
pub async fn job_get(state: State<'_, AppState>, id: Uuid) -> Result<JobDto, AppError> {
    let job = state.jobs.get(id).await?;
    Ok(JobDto::from(&job))
}

#[tauri::command]
pub async fn job_cancel(state: State<'_, AppState>, id: Uuid) -> Result<JobDto, AppError> {
    let job = state.jobs.cancel(id).await?;
    Ok(JobDto::from(&job))
}

#[tauri::command]
pub async fn job_subscribe(
    state: State<'_, AppState>,
    id: Uuid,
    on_event: Channel<JobEvent>,
) -> Result<(), AppError> {
    let mut receiver = state.jobs.subscribe(id).await?;
    tauri::async_runtime::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    if on_event.send(event).is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    Ok(())
}
