use crate::auth::AuthenticatedUser;
use crate::error::ApiError;
use crate::services::devices;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use uuid::Uuid;
use verruckt_cloud_protocol::auth::DeviceDto;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/me/devices", get(list_devices))
        .route("/api/v1/me/devices/{id}", delete(delete_device))
}

async fn list_devices(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
) -> Result<Json<Vec<DeviceDto>>, ApiError> {
    Ok(Json(
        devices::list_devices(&state, authenticated.user_id).await?,
    ))
}

async fn delete_device(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    Path(device_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    devices::delete_device(&state, authenticated.user_id, device_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
