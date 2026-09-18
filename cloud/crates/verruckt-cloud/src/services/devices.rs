use crate::error::ApiError;
use crate::state::AppState;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use verruckt_cloud_protocol::auth::DeviceDto;

#[derive(sqlx::FromRow)]
struct DeviceRow {
    id: Uuid,
    name: String,
    platform: String,
    app_version: String,
    created_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
}

impl From<DeviceRow> for DeviceDto {
    fn from(row: DeviceRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            platform: row.platform,
            app_version: row.app_version,
            created_at: row.created_at,
            last_seen_at: row.last_seen_at,
        }
    }
}

pub async fn list_devices(state: &AppState, user_id: Uuid) -> Result<Vec<DeviceDto>, ApiError> {
    let devices = sqlx::query_as::<_, DeviceRow>(
        r#"
        SELECT id, name, platform, app_version, created_at, last_seen_at
        FROM devices
        WHERE user_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(devices.into_iter().map(DeviceDto::from).collect())
}

pub async fn delete_device(
    state: &AppState,
    user_id: Uuid,
    device_id: Uuid,
) -> Result<(), ApiError> {
    let result = sqlx::query("DELETE FROM devices WHERE id = $1 AND user_id = $2")
        .bind(device_id)
        .bind(user_id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("device_not_found", "device not found"));
    }
    Ok(())
}
