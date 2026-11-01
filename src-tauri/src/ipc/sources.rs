use super::dto::{
    ImportBytesInput, ImportFilesInput, ImportUrlInput, JobDto, SourceDto, SourceSummaryDto,
};
use crate::domain::SourceKind;
use crate::error::AppError;
use crate::ipc::State;
use crate::services::ingestion_service::{IngestInput, IngestionService};
use crate::services::source_service;
use crate::state::AppState;
use base64::Engine as _;
use uuid::Uuid;

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn source_list(state: State<'_, AppState>) -> Result<Vec<SourceDto>, AppError> {
    let sources = source_service::list_sources(&state.pool).await?;
    Ok(sources.iter().map(SourceDto::from).collect())
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn source_summarize(
    state: State<'_, AppState>,
    id: Uuid,
) -> Result<SourceSummaryDto, AppError> {
    let result = source_service::summarize_source(&state, id).await?;
    Ok(SourceSummaryDto {
        source_id: id,
        summary: result.summary,
        key_points: result.key_points,
        cached: result.cached,
    })
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn source_import_bytes(
    state: State<'_, AppState>,
    input: ImportBytesInput,
) -> Result<JobDto, AppError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(input.bytes_base64.trim())
        .map_err(|error| AppError::InvalidInput(format!("invalid base64 payload: {error}")))?;
    let ingest_input = IngestInput {
        filename: input.filename,
        bytes,
        kind: SourceKind::File,
        title: input.title,
        uri: None,
        mime_type: input.mime_type,
    };
    let state = state.inner().clone();
    let jobs = state.jobs.clone();
    let job = jobs
        .start("source_ingestion", move |handle| async move {
            let source = IngestionService::new(&state)
                .ingest_bytes(ingest_input, Some(&handle))
                .await?;
            record_source_result(&state, &handle, source.id).await
        })
        .await?;
    Ok(JobDto::from(&job))
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn source_import_files(
    state: State<'_, AppState>,
    input: ImportFilesInput,
) -> Result<Vec<JobDto>, AppError> {
    if input.paths.is_empty() {
        return Err(AppError::InvalidInput(
            "at least one file path is required".into(),
        ));
    }
    let mut jobs = Vec::with_capacity(input.paths.len());
    for (index, path) in input.paths.into_iter().enumerate() {
        let filename = std::path::Path::new(&path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("document")
            .to_owned();
        let title = input
            .titles
            .as_ref()
            .and_then(|titles| titles.get(index))
            .cloned();
        use tokio::io::AsyncReadExt;
        let file = tokio::fs::File::open(&path).await?;
        let mut bytes = Vec::new();
        file.take(50 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .await?;
        if bytes.len() > 50 * 1024 * 1024 {
            return Err(AppError::InvalidInput("file exceeds 50 MiB limit".into()));
        }
        let state = state.inner().clone();
        let ingest_input = IngestInput {
            filename,
            bytes,
            kind: SourceKind::File,
            title,
            uri: None,
            mime_type: None,
        };
        let job_manager = state.jobs.clone();
        let job = job_manager
            .start("source_ingestion", move |handle| async move {
                IngestionService::new(&state)
                    .ingest_bytes(ingest_input, Some(&handle))
                    .await
                    .map(|_| ())
            })
            .await?;
        jobs.push(JobDto::from(&job));
    }
    Ok(jobs)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn source_import_url(
    state: State<'_, AppState>,
    input: ImportUrlInput,
) -> Result<JobDto, AppError> {
    let state = state.inner().clone();
    let jobs = state.jobs.clone();
    let job = jobs
        .start("source_ingestion", move |handle| async move {
            let source = IngestionService::new(&state)
                .ingest_url(&input.url, input.title, Some(&handle))
                .await?;
            record_source_result(&state, &handle, source.id).await
        })
        .await?;
    Ok(JobDto::from(&job))
}

/// Publish the new source id on the job so the creation flow can attach it to
/// the route it is about to build.
async fn record_source_result(
    state: &AppState,
    handle: &crate::jobs::manager::JobHandle,
    source_id: Uuid,
) -> Result<(), AppError> {
    crate::db::repo::jobs::set_job_result(
        &state.pool,
        handle.id(),
        &serde_json::json!({ "sourceId": source_id }).to_string(),
    )
    .await
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn source_delete(state: State<'_, AppState>, id: Uuid) -> Result<(), AppError> {
    source_service::delete_source(state.inner(), id).await
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn source_reindex(state: State<'_, AppState>, id: Uuid) -> Result<JobDto, AppError> {
    source_service::get_source(&state.pool, id).await?;
    let state = state.inner().clone();
    let jobs = state.jobs.clone();
    let job = jobs
        .start("source_reindex", move |handle| async move {
            IngestionService::new(&state)
                .reindex_source(id, Some(&handle))
                .await
        })
        .await?;
    Ok(JobDto::from(&job))
}
