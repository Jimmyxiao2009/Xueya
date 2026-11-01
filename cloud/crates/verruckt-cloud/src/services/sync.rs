use crate::error::ApiError;
use crate::state::AppState;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;
use verruckt_cloud_domain::sync::{decide_revision, ChangeOperation, RevisionDecision};
use verruckt_cloud_protocol::sync::{
    SyncAccepted, SyncAckResponse, SyncBootstrapResponse, SyncChangeInput, SyncConflict,
    SyncEntitySnapshot, SyncPullChange, SyncPullResponse, SyncPushRequest, SyncPushResponse,
};

const DEFAULT_PULL_LIMIT: i64 = 100;
const MAX_PULL_LIMIT: i64 = 500;
const DEFAULT_BOOTSTRAP_LIMIT: i64 = 200;
const MAX_BOOTSTRAP_LIMIT: i64 = 500;

#[derive(sqlx::FromRow)]
struct EntityRow {
    revision: i64,
    payload: Option<Value>,
    deleted: bool,
    updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct ChangeRow {
    cursor: i64,
    device_id: Option<Uuid>,
    change_id: Uuid,
    entity_type: String,
    entity_id: Uuid,
    revision: i64,
    operation: String,
    payload: Option<Value>,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct SnapshotRow {
    entity_type: String,
    entity_id: Uuid,
    revision: i64,
    deleted: bool,
    payload: Option<Value>,
    updated_at: DateTime<Utc>,
}

struct ValidatedChange {
    operation: ChangeOperation,
    payload: Value,
}

pub async fn push(
    state: &AppState,
    user_id: Uuid,
    request: SyncPushRequest,
) -> Result<SyncPushResponse, ApiError> {
    if request.changes.len() > 500 {
        return Err(ApiError::bad_request(
            "too_many_changes",
            "a single push may contain at most 500 changes",
        ));
    }

    let device_owned = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM devices WHERE id = $1 AND user_id = $2)",
    )
    .bind(request.device_id)
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;
    if !device_owned {
        return Err(ApiError::not_found("device_not_found", "device not found"));
    }

    let mut accepted = Vec::with_capacity(request.changes.len());
    let mut conflicts = Vec::new();
    for change in request.changes {
        match push_one(state, user_id, request.device_id, change).await? {
            PushOutcome::Accepted(value) => accepted.push(value),
            PushOutcome::Conflict(value) => conflicts.push(value),
        }
    }

    let max_cursor = max_cursor(&state.pool, user_id).await?;
    Ok(SyncPushResponse {
        accepted,
        conflicts,
        new_cursor: encode_cursor(max_cursor),
    })
}

enum PushOutcome {
    Accepted(SyncAccepted),
    Conflict(SyncConflict),
}

async fn push_one(
    state: &AppState,
    user_id: Uuid,
    device_id: Uuid,
    change: SyncChangeInput,
) -> Result<PushOutcome, ApiError> {
    validate_change(&change)?;
    let validated = validate_change_payload(&change)?;

    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    if let Some(existing) = sqlx::query_as::<_, (String, Uuid, i64, String)>(
        "SELECT entity_type, entity_id, revision, operation FROM sync_changes WHERE user_id = $1 AND change_id = $2",
    )
    .bind(user_id)
    .bind(change.change_id)
    .fetch_optional(&mut *tx)
    .await?
    {
        tx.commit().await?;
        return Ok(PushOutcome::Accepted(SyncAccepted {
            change_id: change.change_id,
            entity_type: existing.0,
            entity_id: existing.1,
            revision: existing.2,
            already_applied: true,
        }));
    }

    let current = sqlx::query_as::<_, EntityRow>(
        r#"
        SELECT revision, payload, deleted, updated_at
        FROM sync_entities
        WHERE user_id = $1 AND entity_type = $2 AND entity_id = $3
        FOR UPDATE
        "#,
    )
    .bind(user_id)
    .bind(&change.entity_type)
    .bind(change.entity_id)
    .fetch_optional(&mut *tx)
    .await?;

    if decide_revision(
        current.as_ref().map(|row| row.revision),
        change.base_revision,
    ) == RevisionDecision::Conflict
    {
        let conflict = SyncConflict {
            change_id: change.change_id,
            entity_type: change.entity_type.clone(),
            entity_id: change.entity_id,
            server_version: current.as_ref().map(|row| row.revision).unwrap_or(0),
            client_version: change.revision,
            server_payload: current
                .as_ref()
                .and_then(|row| row.payload.clone())
                .unwrap_or(Value::Null),
            server_updated_at: current
                .as_ref()
                .map(|row| row.updated_at)
                .unwrap_or_else(Utc::now),
            deleted: current.as_ref().map(|row| row.deleted).unwrap_or(false),
        };
        tx.rollback().await?;
        return Ok(PushOutcome::Conflict(conflict));
    }

    let deleted = validated.operation == ChangeOperation::Delete;
    let payload = if deleted {
        Value::Null
    } else {
        validated.payload
    };

    sqlx::query(
        r#"
        INSERT INTO sync_entities
            (user_id, entity_type, entity_id, revision, payload, deleted, updated_at, updated_by_device_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (user_id, entity_type, entity_id) DO UPDATE
        SET revision = EXCLUDED.revision,
            payload = EXCLUDED.payload,
            deleted = EXCLUDED.deleted,
            updated_at = EXCLUDED.updated_at,
            updated_by_device_id = EXCLUDED.updated_by_device_id
        "#,
    )
    .bind(user_id)
    .bind(&change.entity_type)
    .bind(change.entity_id)
    .bind(change.revision)
    .bind(&payload)
    .bind(deleted)
    .bind(change.updated_at)
    .bind(device_id)
    .execute(&mut *tx)
    .await?;

    let inserted = sqlx::query(
        r#"
        INSERT INTO sync_changes
            (user_id, device_id, change_id, entity_type, entity_id, revision, operation, payload)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (user_id, change_id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(device_id)
    .bind(change.change_id)
    .bind(&change.entity_type)
    .bind(change.entity_id)
    .bind(change.revision)
    .bind(operation_str(validated.operation))
    .bind(&payload)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(PushOutcome::Accepted(SyncAccepted {
        change_id: change.change_id,
        entity_type: change.entity_type,
        entity_id: change.entity_id,
        revision: change.revision,
        already_applied: inserted.rows_affected() == 0,
    }))
}

pub async fn pull(
    state: &AppState,
    user_id: Uuid,
    cursor: Option<&str>,
    limit: Option<i64>,
) -> Result<SyncPullResponse, ApiError> {
    let cursor = match cursor {
        Some(value) if !value.trim().is_empty() => decode_cursor(value)?,
        _ => 0,
    };
    let limit = limit.unwrap_or(DEFAULT_PULL_LIMIT).clamp(1, MAX_PULL_LIMIT);

    let rows = sqlx::query_as::<_, ChangeRow>(
        r#"
        SELECT cursor, device_id, change_id, entity_type, entity_id, revision, operation, payload, created_at
        FROM sync_changes
        WHERE user_id = $1 AND cursor > $2
        ORDER BY cursor ASC
        LIMIT $3
        "#,
    )
    .bind(user_id)
    .bind(cursor)
    .bind(limit + 1)
    .fetch_all(&state.pool)
    .await?;

    let has_more = rows.len() as i64 > limit;
    let rows = if has_more {
        rows.into_iter().take(limit as usize).collect::<Vec<_>>()
    } else {
        rows
    };

    let next_cursor = rows
        .last()
        .map(|row| encode_cursor(row.cursor))
        .unwrap_or_else(|| encode_cursor(cursor));

    let changes = rows
        .into_iter()
        .map(|row| SyncPullChange {
            cursor: encode_cursor(row.cursor),
            device_id: row.device_id,
            change_id: row.change_id,
            entity_type: row.entity_type,
            entity_id: row.entity_id,
            revision: row.revision,
            operation: row.operation,
            payload: row.payload.unwrap_or(Value::Null),
            created_at: row.created_at,
        })
        .collect();

    Ok(SyncPullResponse {
        changes,
        next_cursor,
        has_more,
    })
}

pub async fn bootstrap(
    state: &AppState,
    user_id: Uuid,
    cursor: Option<&str>,
    limit: Option<i64>,
) -> Result<SyncBootstrapResponse, ApiError> {
    let (offset, watermark): (i64, i64) = match cursor {
        Some(value) if !value.trim().is_empty() => {
            let bytes = URL_SAFE_NO_PAD
                .decode(value)
                .map_err(|_| ApiError::bad_request("invalid_cursor", "invalid bootstrap cursor"))?;
            serde_json::from_slice(&bytes)
                .map_err(|_| ApiError::bad_request("invalid_cursor", "invalid bootstrap cursor"))?
        }
        _ => (0, max_cursor(&state.pool, user_id).await?),
    };
    if offset < 0 || watermark < 0 {
        return Err(ApiError::bad_request(
            "invalid_cursor",
            "bootstrap cursor cannot be negative",
        ));
    }
    let limit = limit
        .unwrap_or(DEFAULT_BOOTSTRAP_LIMIT)
        .clamp(1, MAX_BOOTSTRAP_LIMIT);

    let rows = sqlx::query_as::<_, SnapshotRow>(
        r#"
        SELECT entity_type, entity_id, revision, operation = 'delete' AS deleted, payload, created_at AS updated_at
        FROM (SELECT DISTINCT ON (entity_type, entity_id) * FROM sync_changes
          WHERE user_id = $1 AND cursor <= $4 ORDER BY entity_type, entity_id, cursor DESC) snapshot
        ORDER BY entity_type ASC, entity_id ASC
        OFFSET $2
        LIMIT $3
        "#,
    )
    .bind(user_id)
    .bind(offset)
    .bind(limit + 1)
    .bind(watermark)
    .fetch_all(&state.pool)
    .await?;

    let has_more = rows.len() as i64 > limit;
    let rows = if has_more {
        rows.into_iter().take(limit as usize).collect::<Vec<_>>()
    } else {
        rows
    };

    let next_cursor = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&(offset + rows.len() as i64, watermark))
            .map_err(|_| ApiError::bad_request("invalid_cursor", "invalid bootstrap cursor"))?,
    );
    let sync_cursor = encode_cursor(watermark);

    let entities = rows
        .into_iter()
        .map(|row| SyncEntitySnapshot {
            entity_type: row.entity_type,
            entity_id: row.entity_id,
            revision: row.revision,
            deleted: row.deleted,
            payload: row.payload.unwrap_or(Value::Null),
            updated_at: row.updated_at,
        })
        .collect();

    Ok(SyncBootstrapResponse {
        entities,
        next_cursor,
        has_more,
        sync_cursor,
    })
}

pub async fn ack(
    state: &AppState,
    user_id: Uuid,
    device_id: Uuid,
    cursor: &str,
) -> Result<SyncAckResponse, ApiError> {
    let cursor = decode_cursor(cursor)?;
    let result = sqlx::query(
        r#"
        UPDATE devices
        SET last_sync_cursor = GREATEST(last_sync_cursor, $3)
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(device_id)
    .bind(user_id)
    .bind(cursor)
    .execute(&state.pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("device_not_found", "device not found"));
    }
    Ok(SyncAckResponse {
        acknowledged_cursor: encode_cursor(cursor),
    })
}

async fn max_cursor(pool: &sqlx::PgPool, user_id: Uuid) -> Result<i64, ApiError> {
    let cursor = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(cursor), 0) FROM sync_changes WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(cursor)
}

fn validate_change(change: &SyncChangeInput) -> Result<(), ApiError> {
    if change.entity_type.trim().is_empty() || change.entity_type.len() > 128 {
        return Err(ApiError::bad_request(
            "invalid_entity_type",
            "entity_type must be between 1 and 128 characters",
        ));
    }
    if change.base_revision < 0 {
        return Err(ApiError::bad_request(
            "invalid_base_revision",
            "base_revision must be zero or greater",
        ));
    }
    if change.revision <= 0 {
        return Err(ApiError::bad_request(
            "invalid_revision",
            "revision must be greater than zero",
        ));
    }
    if change.revision <= change.base_revision {
        return Err(ApiError::bad_request(
            "invalid_revision",
            "revision must be greater than base_revision",
        ));
    }
    Ok(())
}

fn validate_change_payload(change: &SyncChangeInput) -> Result<ValidatedChange, ApiError> {
    let operation = match change.operation.as_str() {
        "upsert" => ChangeOperation::Upsert,
        "delete" => ChangeOperation::Delete,
        _ => {
            return Err(ApiError::bad_request(
                "invalid_operation",
                "operation must be 'upsert' or 'delete'",
            ));
        }
    };
    Ok(ValidatedChange {
        operation,
        payload: change.payload.clone(),
    })
}

fn operation_str(operation: ChangeOperation) -> &'static str {
    match operation {
        ChangeOperation::Upsert => "upsert",
        ChangeOperation::Delete => "delete",
    }
}

pub fn encode_cursor(cursor: i64) -> String {
    URL_SAFE_NO_PAD.encode(cursor.to_be_bytes())
}

pub fn decode_cursor(cursor: &str) -> Result<i64, ApiError> {
    let bytes = URL_SAFE_NO_PAD.decode(cursor.trim()).map_err(|_| {
        ApiError::bad_request("invalid_cursor", "cursor is not a valid opaque cursor")
    })?;
    let bytes: [u8; 8] = bytes.try_into().map_err(|_| {
        ApiError::bad_request("invalid_cursor", "cursor is not a valid opaque cursor")
    })?;
    Ok(i64::from_be_bytes(bytes))
}

#[allow(dead_code)]
async fn _assert_transaction_type(_tx: &mut Transaction<'_, Postgres>) {}
