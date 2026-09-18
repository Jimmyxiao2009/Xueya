use crate::error::AppError;
use crate::ipc::dto::{SettingsDto, SettingsUpdateInput};
use crate::state::AppState;
use crate::util::time::utc_now;
use sqlx::SqlitePool;

const SEARCH_PROVIDER_KEY: &str = "search_provider";
const BRAVE_SECRET_REF: &str = "secret://search/brave";
const AI_MODE_KEY: &str = "ai_provider_mode";

pub async fn initialize(state: &AppState) -> Result<(), AppError> {
    if get_setting_string(&state.pool, AI_MODE_KEY)
        .await?
        .is_none()
    {
        let byok_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM providers WHERE enabled = 1 AND base_url NOT LIKE 'mock://%'",
        )
        .fetch_one(&state.pool)
        .await?;
        let mode = if byok_count > 0 { "byok" } else { "offline" };
        set_setting_string(&state.pool, AI_MODE_KEY, mode).await?;
    }
    let settings = get_settings(state).await?;
    if settings.has_brave_api_key {
        if let Ok(api_key) = state.secrets.get_secret(BRAVE_SECRET_REF).await {
            state.search.configure_brave(api_key)?;
        }
    }
    Ok(())
}

pub async fn get_settings(state: &AppState) -> Result<SettingsDto, AppError> {
    let search_provider = get_setting_string(&state.pool, SEARCH_PROVIDER_KEY)
        .await?
        .unwrap_or_else(|| "brave".into());
    let has_brave_api_key = state.secrets.secret_exists(BRAVE_SECRET_REF).await?;
    let ai_mode = get_setting_string(&state.pool, AI_MODE_KEY)
        .await?
        .unwrap_or_else(|| "offline".into());
    Ok(SettingsDto {
        search_provider,
        has_brave_api_key,
        ai_mode,
    })
}

pub async fn update_settings(
    state: &AppState,
    input: SettingsUpdateInput,
) -> Result<SettingsDto, AppError> {
    if let Some(provider) = input.search_provider.as_deref() {
        if !matches!(provider, "brave" | "none") {
            return Err(AppError::InvalidInput(
                "search provider must be 'brave' or 'none'".into(),
            ));
        }
        set_setting_string(&state.pool, SEARCH_PROVIDER_KEY, provider).await?;
    }
    if let Some(api_key) = input.brave_api_key {
        let trimmed = api_key.trim();
        if trimmed.is_empty() {
            state.secrets.delete_secret(BRAVE_SECRET_REF).await?;
            state.search.clear()?;
        } else {
            state.secrets.set_secret(BRAVE_SECRET_REF, trimmed).await?;
            state.search.configure_brave(trimmed)?;
        }
    }
    if let Some(mode) = input.ai_mode.as_deref() {
        if !matches!(mode, "offline" | "byok" | "cloud") {
            return Err(AppError::InvalidInput(
                "AI mode must be 'offline', 'byok' or 'cloud'".into(),
            ));
        }
        set_setting_string(&state.pool, AI_MODE_KEY, mode).await?;
    }
    get_settings(state).await
}

async fn get_setting_string(pool: &SqlitePool, key: &str) -> Result<Option<String>, AppError> {
    let value: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = ?")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(value.and_then(|value| serde_json::from_str(&value).ok()))
}

async fn set_setting_string(pool: &SqlitePool, key: &str, value: &str) -> Result<(), AppError> {
    let json =
        serde_json::to_string(value).map_err(|error| AppError::Internal(error.to_string()))?;
    sqlx::query(
        "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?, ?, ?) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(json)
    .bind(utc_now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}
