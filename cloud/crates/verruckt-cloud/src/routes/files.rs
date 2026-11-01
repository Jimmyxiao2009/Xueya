use crate::auth::AuthenticatedUser;
use crate::error::ApiError;
use crate::services::files;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::{delete, post};
use axum::{Json, Router};
use uuid::Uuid;
use verruckt_cloud_protocol::files::{
    DeleteFileResponse, PresignDownloadRequest, PresignDownloadResponse, PresignUploadRequest,
    PresignUploadResponse,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/files/presign-upload", post(presign_upload))
        .route("/api/v1/files/presign-download", post(presign_download))
        .route("/api/v1/files/{id}/complete", post(complete_upload))
        .route("/api/v1/files/{id}", delete(delete_file))
}

async fn presign_upload(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    Json(request): Json<PresignUploadRequest>,
) -> Result<Json<PresignUploadResponse>, ApiError> {
    Ok(Json(
        files::presign_upload(&state, authenticated.user_id, request).await?,
    ))
}

async fn presign_download(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    Json(request): Json<PresignDownloadRequest>,
) -> Result<Json<PresignDownloadResponse>, ApiError> {
    Ok(Json(
        files::presign_download(&state, authenticated.user_id, request.file_id).await?,
    ))
}

async fn delete_file(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    Path(file_id): Path<Uuid>,
) -> Result<Json<DeleteFileResponse>, ApiError> {
    Ok(Json(
        files::delete_file(&state, authenticated.user_id, file_id).await?,
    ))
}

async fn complete_upload(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    Path(file_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    files::complete_upload(&state, authenticated.user_id, file_id).await?;
    Ok(Json(serde_json::json!({"complete": true})))
}
