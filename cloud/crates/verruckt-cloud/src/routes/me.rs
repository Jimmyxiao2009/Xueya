use crate::auth::{self, AuthenticatedUser};
use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use verruckt_cloud_protocol::auth::MeResponse;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/me", get(me))
}

async fn me(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
) -> Result<Json<MeResponse>, ApiError> {
    Ok(Json(auth::me(&state, authenticated.user_id).await?))
}
