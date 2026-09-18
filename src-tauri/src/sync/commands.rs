use super::engine::{SyncConflictDto, SyncStatusDto, SyncSummary};
use super::files::FileSyncSummary;
use crate::error::AppError;
use crate::services::ingestion_service::IngestionService;
use crate::state::AppState;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn sync_status(state: State<'_, AppState>) -> Result<SyncStatusDto, AppError> {
    state.sync.status().await
}

/// Manual "sync now" from the account UI. A failed sync is reported as an
/// error, but the local outbox is untouched and the next trigger retries.
#[tauri::command]
pub async fn sync_now(state: State<'_, AppState>) -> Result<SyncSummary, AppError> {
    state.sync.sync_now().await
}

#[tauri::command]
pub async fn sync_conflicts(state: State<'_, AppState>) -> Result<Vec<SyncConflictDto>, AppError> {
    state.sync.conflicts().await
}

/// Upload/download source files, then rebuild derived chunks and embeddings
/// locally. Embeddings are deliberately never synced.
#[tauri::command]
pub async fn sync_files_now(state: State<'_, AppState>) -> Result<FileSyncSummary, AppError> {
    let summary = state.files.sync().await?;
    for source_id in &summary.reindex_source_ids {
        if let Ok(source_id) = Uuid::parse_str(source_id) {
            if let Err(error) = IngestionService::new(state.inner())
                .reindex_source(source_id, None)
                .await
            {
                tracing::warn!(%source_id, %error, "could not reindex a downloaded source");
            }
        }
    }
    Ok(summary)
}
