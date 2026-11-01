//! Development-only diagnostics for the vector backend.
//!
//! Backend-only command surface (`vector_backend_info`): local engine stats
//! for the dev retrieval inspector. No telemetry, no upload.

use crate::ipc::State;
use serde::Serialize;

use crate::error::AppError;
use crate::knowledge::vector_index::VectorBackendStats;
use crate::state::{AppState, VectorBackend};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorBackendInfo {
    pub backend: String,
    pub algorithm: Option<String>,
    pub vector_count: Option<u64>,
    pub deleted_count: Option<u64>,
    pub dimension: Option<u32>,
    pub index_bytes: Option<u64>,
}

/// Returns a diagnostics snapshot of the selected vector backend. The
/// reported backend mirrors the process-wide `VERRUCKT_VECTOR_BACKEND`
/// selection rather than guessing from the trait object.
#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn vector_backend_info(
    state: State<'_, AppState>,
) -> Result<VectorBackendInfo, AppError> {
    let backend = VectorBackend::from_env();
    let stats = state.vector_index.info().await;
    Ok(match (backend, stats) {
        (VectorBackend::Sqlite, Some(stats)) => info("sqlite", stats),
        (VectorBackend::Sqlite, None) => VectorBackendInfo {
            backend: "sqlite".to_owned(),
            algorithm: None,
            vector_count: None,
            deleted_count: None,
            dimension: None,
            index_bytes: None,
        },
        (VectorBackend::Verruckt, Some(stats)) => info("verruckt", stats),
        (VectorBackend::Verruckt, None) => VectorBackendInfo {
            backend: "verruckt".to_owned(),
            algorithm: None,
            vector_count: None,
            deleted_count: None,
            dimension: None,
            index_bytes: None,
        },
    })
}

fn info(backend: &str, stats: VectorBackendStats) -> VectorBackendInfo {
    VectorBackendInfo {
        backend: backend.to_owned(),
        algorithm: Some(stats.algorithm),
        vector_count: Some(stats.vector_count),
        deleted_count: stats.deleted_count,
        dimension: Some(stats.dimension).filter(|dimension| *dimension > 0),
        index_bytes: stats.index_bytes,
    }
}
