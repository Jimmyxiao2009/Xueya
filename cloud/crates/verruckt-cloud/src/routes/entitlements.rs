use crate::auth::AuthenticatedUser;
use crate::error::ApiError;
use crate::services::entitlements;
use crate::state::AppState;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use verruckt_cloud_protocol::entitlements::EntitlementsResponse;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/me/entitlements", get(get_entitlements))
}

async fn get_entitlements(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
) -> Result<Json<EntitlementsResponse>, ApiError> {
    Ok(Json(
        entitlements::get_entitlements_response(&state, authenticated.user_id).await?,
    ))
}
