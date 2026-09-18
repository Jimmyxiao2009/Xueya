use crate::auth::AuthenticatedUser;
use crate::error::ApiError;
use crate::services::entitlements::resolve_entitlements;
use crate::services::sync;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use verruckt_cloud_protocol::sync::{
    SyncAckRequest, SyncAckResponse, SyncBootstrapResponse, SyncPullResponse, SyncPushRequest,
    SyncPushResponse,
};

#[derive(Debug, Deserialize)]
struct PullQuery {
    cursor: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct BootstrapQuery {
    cursor: Option<String>,
    limit: Option<i64>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/sync/push", post(push))
        .route("/api/v1/sync/pull", get(pull))
        .route("/api/v1/sync/bootstrap", get(bootstrap))
        .route("/api/v1/sync/ack", post(ack))
}

async fn push(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    Json(request): Json<SyncPushRequest>,
) -> Result<Json<SyncPushResponse>, ApiError> {
    require_cloud_sync(&state, authenticated.user_id).await?;
    Ok(Json(
        sync::push(&state, authenticated.user_id, request).await?,
    ))
}

async fn pull(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    Query(query): Query<PullQuery>,
) -> Result<Json<SyncPullResponse>, ApiError> {
    require_cloud_sync(&state, authenticated.user_id).await?;
    Ok(Json(
        sync::pull(
            &state,
            authenticated.user_id,
            query.cursor.as_deref(),
            query.limit,
        )
        .await?,
    ))
}

async fn bootstrap(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    Query(query): Query<BootstrapQuery>,
) -> Result<Json<SyncBootstrapResponse>, ApiError> {
    require_cloud_sync(&state, authenticated.user_id).await?;
    Ok(Json(
        sync::bootstrap(
            &state,
            authenticated.user_id,
            query.cursor.as_deref(),
            query.limit,
        )
        .await?,
    ))
}

async fn ack(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    Json(request): Json<SyncAckRequest>,
) -> Result<Json<SyncAckResponse>, ApiError> {
    require_cloud_sync(&state, authenticated.user_id).await?;
    Ok(Json(
        sync::ack(
            &state,
            authenticated.user_id,
            request.device_id,
            &request.cursor,
        )
        .await?,
    ))
}

async fn require_cloud_sync(state: &AppState, user_id: uuid::Uuid) -> Result<(), ApiError> {
    let entitlements = resolve_entitlements(&state.pool, user_id).await?;
    if !entitlements.cloud_sync_enabled() {
        return Err(ApiError::forbidden(
            "entitlement_required",
            "cloud_sync entitlement is required",
        ));
    }
    Ok(())
}
