use crate::error::ApiError;
use crate::services::entitlements::resolve_entitlements;
use crate::state::AppState;
use uuid::Uuid;
use verruckt_cloud_protocol::files::{
    DeleteFileResponse, PresignDownloadResponse, PresignUploadRequest, PresignUploadResponse,
};
use verruckt_cloud_storage::S3ObjectStorage;

#[derive(sqlx::FromRow)]
struct ObjectRow {
    id: Uuid,
    size: i64,
    object_key: String,
}

pub async fn presign_upload(
    state: &AppState,
    user_id: Uuid,
    request: PresignUploadRequest,
) -> Result<PresignUploadResponse, ApiError> {
    let storage = require_storage(state)?;
    validate_upload_request(state, &request)?;

    let entitlements = resolve_entitlements(&state.pool, user_id).await?;
    if !entitlements.cloud_sync_enabled() {
        return Err(ApiError::forbidden(
            "entitlement_required",
            "cloud_sync entitlement is required",
        ));
    }
    let limit = entitlements.max_cloud_storage_bytes();
    if limit <= 0 || request.size > limit {
        return Err(ApiError::payment_required(
            "storage_quota_exceeded",
            "cloud storage quota exceeded",
        ));
    }

    let mut tx = state.pool.begin().await?;
    let existing = sqlx::query_as::<_, ObjectRow>(
        r#"
        SELECT id, size, object_key
        FROM objects
        WHERE user_id = $1 AND sha256 = $2
        "#,
    )
    .bind(user_id)
    .bind(&request.sha256)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(existing) = existing {
        if existing.size != request.size {
            return Err(ApiError::conflict(
                "sha256_size_mismatch",
                "an object with this sha256 already exists with a different size",
            ));
        }
        let presigned = storage.presign_put(&existing.object_key, Some(&request.mime))?;
        tx.commit().await?;
        return Ok(PresignUploadResponse {
            file_id: existing.id,
            object_key: existing.object_key,
            upload_url: presigned.url,
            method: presigned.method,
            headers: presigned.headers,
            expires_in: presigned.expires_in,
            deduplicated: true,
        });
    }

    sqlx::query(
        "INSERT INTO user_storage_usage (user_id, bytes_used) VALUES ($1, 0) ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    let bytes_used = sqlx::query_scalar::<_, i64>(
        "SELECT bytes_used FROM user_storage_usage WHERE user_id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    if bytes_used + request.size > limit {
        return Err(ApiError::payment_required(
            "storage_quota_exceeded",
            "cloud storage quota exceeded",
        ));
    }

    let file_id = Uuid::new_v4();
    let object_key = storage.object_key(user_id, &request.sha256);
    sqlx::query(
        r#"
        INSERT INTO objects (id, user_id, sha256, size, mime, filename, object_key, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'active')
        "#,
    )
    .bind(file_id)
    .bind(user_id)
    .bind(&request.sha256)
    .bind(request.size)
    .bind(&request.mime)
    .bind(&request.filename)
    .bind(&object_key)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE user_storage_usage SET bytes_used = bytes_used + $2, updated_at = now() WHERE user_id = $1",
    )
    .bind(user_id)
    .bind(request.size)
    .execute(&mut *tx)
    .await?;

    let presigned = storage.presign_put(&object_key, Some(&request.mime))?;
    tx.commit().await?;

    Ok(PresignUploadResponse {
        file_id,
        object_key,
        upload_url: presigned.url,
        method: presigned.method,
        headers: presigned.headers,
        expires_in: presigned.expires_in,
        deduplicated: false,
    })
}

pub async fn presign_download(
    state: &AppState,
    user_id: Uuid,
    file_id: Uuid,
) -> Result<PresignDownloadResponse, ApiError> {
    let storage = require_storage(state)?;
    let object = sqlx::query_as::<_, ObjectRow>(
        r#"
        SELECT id, size, object_key
        FROM objects
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(file_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("file_not_found", "file not found"))?;

    let presigned = storage.presign_get(&object.object_key)?;
    Ok(PresignDownloadResponse {
        file_id: object.id,
        object_key: object.object_key,
        download_url: presigned.url,
        method: presigned.method,
        expires_in: presigned.expires_in,
    })
}

pub async fn delete_file(
    state: &AppState,
    user_id: Uuid,
    file_id: Uuid,
) -> Result<DeleteFileResponse, ApiError> {
    let storage = require_storage(state)?;
    let object = sqlx::query_as::<_, ObjectRow>(
        r#"
        SELECT id, size, object_key
        FROM objects
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(file_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("file_not_found", "file not found"))?;

    storage.delete_object(&object.object_key).await?;

    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM objects WHERE id = $1 AND user_id = $2")
        .bind(file_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"
        UPDATE user_storage_usage
        SET bytes_used = GREATEST(bytes_used - $2, 0), updated_at = now()
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .bind(object.size)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(DeleteFileResponse {
        deleted: true,
        file_id,
    })
}

fn require_storage(state: &AppState) -> Result<&S3ObjectStorage, ApiError> {
    state.storage.as_deref().ok_or_else(|| {
        ApiError::service_unavailable(
            "object_storage_not_configured",
            "object storage is not configured",
        )
    })
}

fn validate_upload_request(
    state: &AppState,
    request: &PresignUploadRequest,
) -> Result<(), ApiError> {
    if request.sha256.len() != 64 || !request.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ApiError::bad_request(
            "invalid_sha256",
            "sha256 must be a 64-character hexadecimal string",
        ));
    }
    if request.size <= 0 {
        return Err(ApiError::bad_request(
            "invalid_size",
            "size must be greater than zero",
        ));
    }
    if request.size > state.config.max_file_bytes {
        return Err(ApiError::payload_too_large(
            "file_too_large",
            "file exceeds the configured maximum size",
        ));
    }
    if request.mime.trim().is_empty() || request.mime.len() > 255 {
        return Err(ApiError::bad_request(
            "invalid_mime",
            "mime must be between 1 and 255 characters",
        ));
    }
    if let Some(filename) = &request.filename {
        if filename.len() > 512 {
            return Err(ApiError::bad_request(
                "invalid_filename",
                "filename must be at most 512 characters",
            ));
        }
    }
    Ok(())
}
