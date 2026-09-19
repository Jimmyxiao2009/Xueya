use crate::ai::prompts::SOURCE_SUMMARY_SYSTEM;
use crate::ai::schemas::SourceSummary;
use crate::ai::structured::generate_structured;
use crate::ai::traits::ChatMessage;
use crate::db::repo::sources as source_repo;
use crate::domain::{Source, SourceStatus};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::time::utc_now;
use sqlx::SqlitePool;
use uuid::Uuid;

const MAX_SUMMARY_INPUT_CHARS: usize = 24_000;

pub async fn list_sources(pool: &SqlitePool) -> Result<Vec<Source>, AppError> {
    source_repo::list_sources(pool).await
}

pub async fn get_source(pool: &SqlitePool, id: Uuid) -> Result<Source, AppError> {
    source_repo::get_source(pool, id).await
}

pub async fn delete_source(state: &AppState, id: Uuid) -> Result<(), AppError> {
    let chunk_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT c.id FROM chunks c JOIN documents d ON d.id = c.document_id WHERE d.source_id = ?",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    for chunk_id in chunk_ids {
        if let Err(error) = state.vector_index.delete(chunk_id).await {
            tracing::warn!(%error, %chunk_id, "derived vector delete failed during source delete");
        }
    }
    source_repo::delete_source(&state.pool, id)
        .await
        .map_err(|error| error.context("delete source authoritative row"))
}

#[derive(Debug, Clone)]
pub struct SourceSummaryResult {
    pub summary: String,
    pub key_points: Vec<String>,
    pub cached: bool,
}

/// Summarize a source once per content hash and cache the result.
///
/// The cache is keyed by the source's content hash, so re-importing the same
/// bytes does not call the model again. A changed revision produces a new
/// summary and overwrites the old row.
pub async fn summarize_source(
    state: &AppState,
    source_id: Uuid,
) -> Result<SourceSummaryResult, AppError> {
    let source = source_repo::get_source(&state.pool, source_id).await?;
    if source.status != SourceStatus::Ready {
        return Err(AppError::InvalidInput(
            "source must finish importing before it can be summarized".into(),
        ));
    }
    let content_hash = match source.content_hash.clone() {
        Some(hash) if !hash.is_empty() => hash,
        _ => source_id.to_string(),
    };

    let cached: Option<(String, String, String)> = sqlx::query_as(
        "SELECT content_hash, summary, key_points_json FROM source_summaries WHERE source_id = ?",
    )
    .bind(source_id)
    .fetch_optional(&state.pool)
    .await?;
    if let Some((hash, summary, key_points_json)) = cached {
        if hash == content_hash && !summary.trim().is_empty() {
            return Ok(SourceSummaryResult {
                summary,
                key_points: serde_json::from_str(&key_points_json).unwrap_or_default(),
                cached: true,
            });
        }
    }

    let documents = source_repo::list_documents_by_source(&state.pool, source_id).await?;
    let mut text = String::new();
    for document in documents {
        if text.chars().count() >= MAX_SUMMARY_INPUT_CHARS {
            break;
        }
        text.push_str(&document.title);
        text.push('\n');
        let remaining = MAX_SUMMARY_INPUT_CHARS.saturating_sub(text.chars().count());
        text.extend(document.text_content.chars().take(remaining));
        text.push_str("\n\n");
    }
    if text.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "source has no readable text to summarize".into(),
        ));
    }

    let (config, chat) = state.providers.default_chat_provider().await?;
    let messages = vec![
        ChatMessage::system(SOURCE_SUMMARY_SYSTEM),
        ChatMessage::user(format!(
            "Source title: {}\n\nUNTRUSTED DOCUMENT CONTENT\n{text}",
            source.title
        )),
    ];
    let summary = generate_structured::<SourceSummary, _>(
        chat.as_ref(),
        &config.chat_model,
        messages,
        |value| {
            if value.summary.trim().is_empty() {
                return Err(AppError::AiResponseInvalid(
                    "source summary is empty".into(),
                ));
            }
            Ok(())
        },
    )
    .await?;

    let key_points_json = serde_json::to_string(&summary.key_points)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    sqlx::query(
        "INSERT INTO source_summaries (source_id, content_hash, summary, key_points_json, model, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(source_id) DO UPDATE SET \
         content_hash = excluded.content_hash, summary = excluded.summary, key_points_json = excluded.key_points_json, model = excluded.model, updated_at = excluded.updated_at",
    )
    .bind(source_id)
    .bind(&content_hash)
    .bind(&summary.summary)
    .bind(&key_points_json)
    .bind(&config.chat_model)
    .bind(utc_now().to_rfc3339())
    .bind(utc_now().to_rfc3339())
    .execute(&state.pool)
    .await?;

    Ok(SourceSummaryResult {
        summary: summary.summary,
        key_points: summary.key_points,
        cached: false,
    })
}
