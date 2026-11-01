use crate::cloud::models::CloudAccountState;
use crate::domain::AiMode;
use crate::error::AppError;
use crate::ipc::dto::{SettingsDto, SettingsUpdateInput};
use crate::state::AppState;
use crate::util::time::utc_now;
use sqlx::SqlitePool;

const SEARCH_PROVIDER_KEY: &str = "search_provider";
const BRAVE_SECRET_REF: &str = "secret://search/brave";
const TAVILY_SECRET_REF: &str = "secret://search/tavily";
const SERPER_SECRET_REF: &str = "secret://search/serper";
const SEARXNG_URL_KEY: &str = "searxng_url";
const AI_MODE_KEY: &str = "ai_provider_mode";
/// Distinguishes an automatic provider choice from a user's explicit setting.
/// The value is a JSON boolean so older string-only installs remain readable.
const AI_MODE_EXPLICIT_KEY: &str = "ai_provider_preference_explicit";

pub async fn initialize(state: &AppState) -> Result<(), AppError> {
    reconfigure_search(state).await?;
    let account = state.cloud.account_state().await?;
    reconcile_provider_policy(state, &account).await?;
    state.cloud.reconcile_sync_policy(&account).await?;
    Ok(())
}

/// Reconcile the stored AI provider mode with account state and BYOK
/// availability while preserving an explicit user choice.
///
/// This is deliberately small: `ai_provider_mode` remains the single source of
/// truth for the runtime, and `ai_provider_preference_explicit` only records
/// whether the value came from the user or from this policy.
pub async fn reconcile_provider_policy(
    state: &AppState,
    account: &CloudAccountState,
) -> Result<(), AppError> {
    let explicit = get_setting_bool(&state.pool, AI_MODE_EXPLICIT_KEY)
        .await?
        .unwrap_or(false);
    let current = get_setting_string(&state.pool, AI_MODE_KEY)
        .await?
        .map(|value| AiMode::parse(&value));
    let byok_available = valid_byok_count(&state.pool).await? > 0;
    let cloud_available = cloud_ai_available(account);

    let (next, next_explicit) = match (explicit, current) {
        // No explicit preference: account-aware default.
        (false, _) => (
            automatic_ai_mode(cloud_available, byok_available),
            false,
        ),
        // Explicit BYOK/offline stays unless it has become impossible to use.
        (true, Some(AiMode::Byok)) if !byok_available => (
            automatic_ai_mode(cloud_available, byok_available),
            false,
        ),
        // Explicit Cloud stays only while the account can actually use it.
        (true, Some(AiMode::Cloud)) if !cloud_available => (
            automatic_ai_mode(cloud_available, byok_available),
            false,
        ),
        (true, Some(mode)) => (mode, true),
        // A malformed explicit state is treated as automatic.
        (true, None) => (
            automatic_ai_mode(cloud_available, byok_available),
            false,
        ),
    };

    if current != Some(next) {
        set_setting_string(&state.pool, AI_MODE_KEY, next.as_str()).await?;
    }
    if explicit != next_explicit {
        set_setting_bool(&state.pool, AI_MODE_EXPLICIT_KEY, next_explicit).await?;
    }
    Ok(())
}

fn automatic_ai_mode(cloud_available: bool, byok_available: bool) -> AiMode {
    if cloud_available {
        AiMode::Cloud
    } else if byok_available {
        AiMode::Byok
    } else {
        AiMode::Offline
    }
}

fn cloud_ai_available(account: &CloudAccountState) -> bool {
    if account.user_id.is_none() {
        return false;
    }
    let Some(entitlements) = account.entitlements.as_ref() else {
        return false;
    };
    if !entitlements.official_ai {
        return false;
    }
    entitlements.monthly_ai_credits > 0
        && entitlements.monthly_ai_credits_used < entitlements.monthly_ai_credits
}

async fn valid_byok_count(pool: &SqlitePool) -> Result<i64, AppError> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM providers WHERE enabled = 1 AND base_url NOT LIKE 'mock://%'",
    )
    .fetch_one(pool)
    .await?)
}


pub async fn get_settings(state: &AppState) -> Result<SettingsDto, AppError> {
    let search_provider = get_setting_string(&state.pool, SEARCH_PROVIDER_KEY)
        .await?
        .unwrap_or_else(|| "auto".into());
    let has_brave_api_key = state.secrets.secret_exists(BRAVE_SECRET_REF).await?;
    let has_tavily_api_key = state.secrets.secret_exists(TAVILY_SECRET_REF).await?;
    let has_serper_api_key = state.secrets.secret_exists(SERPER_SECRET_REF).await?;
    let searxng_url = get_setting_string(&state.pool, SEARXNG_URL_KEY)
        .await?
        .unwrap_or_default();
    let ai_mode = get_setting_string(&state.pool, AI_MODE_KEY)
        .await?
        .unwrap_or_else(|| "offline".into());
    Ok(SettingsDto {
        search_provider,
        has_brave_api_key,
        has_tavily_api_key,
        has_serper_api_key,
        searxng_url,
        ai_mode,
    })
}

pub async fn update_settings(
    state: &AppState,
    input: SettingsUpdateInput,
) -> Result<SettingsDto, AppError> {
    if let Some(provider) = input.search_provider.as_deref() {
        if !matches!(provider, "auto" | "brave" | "tavily" | "serper" | "searxng" | "none") {
            return Err(AppError::InvalidInput(
                "search provider must be 'auto', 'brave', 'tavily', 'serper', 'searxng' or 'none'".into(),
            ));
        }
        set_setting_string(&state.pool, SEARCH_PROVIDER_KEY, provider).await?;
    }
    if let Some(api_key) = input.brave_api_key {
        let trimmed = api_key.trim();
        if trimmed.is_empty() {
            state.secrets.delete_secret(BRAVE_SECRET_REF).await?;
        } else {
            state.secrets.set_secret(BRAVE_SECRET_REF, trimmed).await?;
        }
    }
    if let Some(api_key) = input.tavily_api_key {
        let trimmed = api_key.trim();
        if trimmed.is_empty() {
            state.secrets.delete_secret(TAVILY_SECRET_REF).await?;
        } else {
            state.secrets.set_secret(TAVILY_SECRET_REF, trimmed).await?;
        }
    }
    if let Some(api_key) = input.serper_api_key {
        let trimmed = api_key.trim();
        if trimmed.is_empty() {
            state.secrets.delete_secret(SERPER_SECRET_REF).await?;
        } else {
            state.secrets.set_secret(SERPER_SECRET_REF, trimmed).await?;
        }
    }
    if let Some(url) = input.searxng_url {
        set_setting_string(&state.pool, SEARXNG_URL_KEY, url.trim()).await?;
    }
    if let Some(mode) = input.ai_mode.as_deref() {
        if !matches!(mode, "offline" | "byok" | "cloud") {
            return Err(AppError::InvalidInput(
                "AI mode must be 'offline', 'byok' or 'cloud'".into(),
            ));
        }
        set_setting_string(&state.pool, AI_MODE_KEY, mode).await?;
        set_setting_bool(&state.pool, AI_MODE_EXPLICIT_KEY, true).await?;
    }
    reconfigure_search(state).await?;
    get_settings(state).await
}

/// Rebuild the active search providers from persisted settings.
///
/// `auto` configures every provider that has a saved credential; an explicit
/// provider configures only that provider. Missing credentials fall back to
/// other saved providers for compatibility with older installs.
async fn reconfigure_search(state: &AppState) -> Result<(), AppError> {
    let selected = get_setting_string(&state.pool, SEARCH_PROVIDER_KEY)
        .await?
        .unwrap_or_else(|| "auto".into());
    state.search.clear()?;
    if selected == "none" {
        return Ok(());
    }

    let tavily = state.secrets.secret_exists(TAVILY_SECRET_REF).await?;
    let brave = state.secrets.secret_exists(BRAVE_SECRET_REF).await?;
    let serper = state.secrets.secret_exists(SERPER_SECRET_REF).await?;
    let searxng_url = get_setting_string(&state.pool, SEARXNG_URL_KEY)
        .await?
        .unwrap_or_default();
    let searxng = !searxng_url.trim().is_empty();

    let wants = |name: &str| selected == "auto" || selected == name;
    let mut configured = false;
    if wants("tavily") && tavily {
        if let Ok(api_key) = state.secrets.get_secret(TAVILY_SECRET_REF).await {
            state.search.configure_tavily(api_key)?;
            configured = true;
        }
    }
    if wants("brave") && brave {
        if let Ok(api_key) = state.secrets.get_secret(BRAVE_SECRET_REF).await {
            state.search.configure_brave(api_key)?;
            configured = true;
        }
    }
    if wants("serper") && serper {
        if let Ok(api_key) = state.secrets.get_secret(SERPER_SECRET_REF).await {
            state.search.configure_serper(api_key)?;
            configured = true;
        }
    }
    if wants("searxng") && searxng {
        state.search.configure_searxng(searxng_url.trim())?;
        configured = true;
    }
    if configured {
        return Ok(());
    }

    // Compatibility fallback for installs that selected one provider before
    // multi-provider search existed.
    if tavily {
        if let Ok(api_key) = state.secrets.get_secret(TAVILY_SECRET_REF).await {
            state.search.configure_tavily(api_key)?;
            return Ok(());
        }
    }
    if brave {
        if let Ok(api_key) = state.secrets.get_secret(BRAVE_SECRET_REF).await {
            state.search.configure_brave(api_key)?;
            return Ok(());
        }
    }
    if serper {
        if let Ok(api_key) = state.secrets.get_secret(SERPER_SECRET_REF).await {
            state.search.configure_serper(api_key)?;
            return Ok(());
        }
    }
    if searxng {
        state.search.configure_searxng(searxng_url.trim())?;
        return Ok(());
    }
    // Reuse only a key explicitly configured for Step's own API host.
    for provider in state.providers.list().await? {
        if provider.enabled
            && url::Url::parse(&provider.base_url)
                .is_ok_and(|u| u.host_str() == Some("api.stepfun.com"))
        {
            if let Some(reference) = provider.api_key_ref.as_deref() {
                if let Ok(key) = state.secrets.get_secret(reference).await {
                    state.search.configure_step(key)?;
                    return Ok(());
                }
            }
        }
    }
    state.search.clear()?;
    Ok(())
}

async fn get_setting_string(pool: &SqlitePool, key: &str) -> Result<Option<String>, AppError> {
    let value: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = ?")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(value.and_then(|value| serde_json::from_str(&value).ok()))
}

async fn get_setting_bool(pool: &SqlitePool, key: &str) -> Result<Option<bool>, AppError> {
    let value: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = ?")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(value.and_then(|value| {
        serde_json::from_str::<bool>(&value)
            .ok()
            .or_else(|| {
                serde_json::from_str::<String>(&value)
                    .ok()
                    .and_then(|text| text.parse::<bool>().ok())
            })
    }))
}

async fn set_setting_bool(pool: &SqlitePool, key: &str, value: bool) -> Result<(), AppError> {
    let json = serde_json::to_string(&value).map_err(|error| AppError::Internal(error.to_string()))?;
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
