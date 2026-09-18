use crate::domain::ProviderConfig;
use crate::error::AppError;
use sqlx::SqlitePool;
use uuid::Uuid;

pub async fn list_providers(pool: &SqlitePool) -> Result<Vec<ProviderConfig>, AppError> {
    Ok(sqlx::query_as::<_, ProviderConfig>(
        "SELECT id, display_name, base_url, chat_model, embedding_model, api_key_ref, protocol, enabled, created_at, updated_at FROM providers ORDER BY created_at",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn get_provider(pool: &SqlitePool, id: Uuid) -> Result<ProviderConfig, AppError> {
    sqlx::query_as::<_, ProviderConfig>(
        "SELECT id, display_name, base_url, chat_model, embedding_model, api_key_ref, protocol, enabled, created_at, updated_at FROM providers WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("provider {id}")))
}

pub async fn first_enabled_provider(pool: &SqlitePool) -> Result<Option<ProviderConfig>, AppError> {
    let provider = sqlx::query_as::<_, ProviderConfig>(
        "SELECT id, display_name, base_url, chat_model, embedding_model, api_key_ref, protocol, enabled, created_at, updated_at FROM providers WHERE enabled = 1 ORDER BY CASE WHEN base_url LIKE 'mock://%' THEN 1 ELSE 0 END, created_at LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(provider)
}

pub async fn upsert_provider(pool: &SqlitePool, provider: &ProviderConfig) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO providers
            (id, display_name, base_url, chat_model, embedding_model, api_key_ref, protocol, enabled, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            display_name = excluded.display_name,
            base_url = excluded.base_url,
            chat_model = excluded.chat_model,
            embedding_model = excluded.embedding_model,
            api_key_ref = COALESCE(excluded.api_key_ref, providers.api_key_ref),
            protocol = excluded.protocol,
            enabled = excluded.enabled,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(provider.id)
    .bind(&provider.display_name)
    .bind(&provider.base_url)
    .bind(&provider.chat_model)
    .bind(&provider.embedding_model)
    .bind(&provider.api_key_ref)
    .bind(provider.protocol)
    .bind(provider.enabled)
    .bind(provider.created_at.to_rfc3339())
    .bind(provider.updated_at.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_provider(pool: &SqlitePool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM providers WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
