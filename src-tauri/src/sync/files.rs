use crate::cloud::CloudAccountClient;
use crate::error::AppError;
use crate::knowledge_fs::blob_store::BlobStore;
use crate::util::hashing::sha256_hex;
use crate::util::time::utc_now;
use reqwest::{Method, StatusCode};
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use uuid::Uuid;

#[cfg(test)]
mod restore_regressions {
    use super::*;
    use crate::services::ingestion_service::{IngestInput, IngestionService};
    use crate::state::{AppPaths, AppState};

    #[tokio::test]
    async fn completed_download_without_documents_is_retried_after_restart() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new_with_secrets(
            AppPaths::new(dir.path()),
            Arc::new(crate::secrets::keyring_store::InMemorySecretStore::new()),
        )
        .await
        .unwrap();
        let source = IngestionService::new(&state)
            .ingest_bytes(
                IngestInput {
                    filename: "restore.txt".into(),
                    bytes: b"Geometry studies points and lines.".to_vec(),
                    kind: crate::domain::SourceKind::File,
                    title: None,
                    uri: None,
                    mime_type: Some("text/plain".into()),
                },
                None,
            )
            .await
            .unwrap();
        let revision: Uuid =
            sqlx::query_scalar("SELECT id FROM source_revisions WHERE source_id = ?")
                .bind(source.id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        let file_id = Uuid::new_v4().to_string();
        sqlx::query("UPDATE source_revisions SET cloud_file_id = ? WHERE id = ?")
            .bind(&file_id)
            .bind(revision)
            .execute(&state.pool)
            .await
            .unwrap();
        state
            .files
            .upsert_file_state(
                revision,
                source.content_hash.as_deref().unwrap(),
                Some(&file_id),
                None,
                "downloaded",
                None,
            )
            .await
            .unwrap();
        sqlx::query("DELETE FROM documents WHERE source_id = ?")
            .bind(source.id)
            .execute(&state.pool)
            .await
            .unwrap();
        let (downloaded, reindex) = state.files.download_missing().await.unwrap();
        assert_eq!(downloaded, 0);
        assert_eq!(reindex, vec![source.id.to_string()]);
        IngestionService::new(&state)
            .reindex_source(source.id, None)
            .await
            .unwrap();
        assert!(state.files.pending_downloads().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn verified_download_replaces_corrupted_existing_blob() {
        let dir = tempfile::tempdir().unwrap();
        let blobs = BlobStore::new(dir.path());
        let saved = blobs.put_bytes(b"verified content").await.unwrap();
        tokio::fs::write(blobs.full_path(&saved.relative_path), b"broken content")
            .await
            .unwrap();
        blobs.put_bytes(b"verified content").await.unwrap();
        assert_eq!(
            blobs.get_bytes(&saved.relative_path).await.unwrap(),
            b"verified content"
        );
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSyncSummary {
    pub uploaded: usize,
    pub downloaded: usize,
    pub reindex_source_ids: Vec<String>,
}

/// Source files are too large for the delta journal. They are uploaded through
/// a presigned object-storage URL and downloaded by another device with
/// SHA-256 verification; embeddings are never synced and are rebuilt locally.
pub struct CloudFileSyncService {
    cloud: Arc<CloudAccountClient>,
    pool: SqlitePool,
    blobs: BlobStore,
    http: reqwest::Client,
}

struct RevisionRow {
    id: Uuid,
    source_id: Uuid,
    content_hash: String,
    blob_path: String,
    mime_type: String,
    size_bytes: i64,
    filename: Option<String>,
    cloud_file_id: Option<String>,
}

impl CloudFileSyncService {
    pub fn new(
        cloud: Arc<CloudAccountClient>,
        pool: SqlitePool,
        blobs: BlobStore,
        http: reqwest::Client,
    ) -> Self {
        Self {
            cloud,
            pool,
            blobs,
            http,
        }
    }

    /// Upload new local revisions and download revisions pulled from another
    /// device. Returns the source ids whose local index must be rebuilt.
    pub async fn sync(&self) -> Result<FileSyncSummary, AppError> {
        let _transition = self.cloud.transition.read().await;
        let account = self.cloud.account_state_unlocked().await?;
        if !account.logged_in
            || !account.sync_enabled
            || !account
                .entitlements
                .as_ref()
                .map(|entitlements| entitlements.cloud_sync)
                .unwrap_or(false)
        {
            return Ok(FileSyncSummary {
                uploaded: 0,
                downloaded: 0,
                reindex_source_ids: Vec::new(),
            });
        }
        self.cloud
            .check_profile_owner(account.user_id.ok_or(AppError::CloudSessionExpired)?, true)
            .await?;
        let uploaded = self.upload_pending().await?;
        let (downloaded, reindex_source_ids) = self.download_missing().await?;
        Ok(FileSyncSummary {
            uploaded,
            downloaded,
            reindex_source_ids,
        })
    }

    async fn upload_pending(&self) -> Result<usize, AppError> {
        let rows = self.pending_uploads().await?;
        let mut uploaded = 0;
        for row in rows {
            match self.upload_one(&row).await {
                Ok(()) => uploaded += 1,
                Err(error) => {
                    self.mark_failed(row.id, &error.to_string()).await;
                    tracing::warn!(revision = %row.id, %error, "cloud file upload failed");
                }
            }
        }
        Ok(uploaded)
    }

    async fn upload_one(&self, row: &RevisionRow) -> Result<(), AppError> {
        let bytes = self.blobs.get_bytes(&row.blob_path).await?;
        let actual_hash = sha256_hex(&bytes);
        if actual_hash != row.content_hash {
            return Err(AppError::Internal(format!(
                "local blob hash mismatch for revision {}",
                row.id
            )));
        }
        let body = json!({
            "sha256": row.content_hash,
            "size": row.size_bytes,
            "mime": row.mime_type,
            "filename": row.filename,
        });
        let response = self
            .cloud
            .authorized_json(Method::POST, "/api/v1/files/presign-upload", Some(&body))
            .await?;
        let presign: Value = response
            .json()
            .await
            .map_err(|error| AppError::CloudUnavailable(error.to_string()))?;
        let file_id = presign
            .get("file_id")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::CloudUnavailable("missing file_id".into()))?;
        let object_key = presign
            .get("object_key")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let deduplicated = presign
            .get("deduplicated")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !deduplicated {
            let upload_url = presign
                .get("upload_url")
                .and_then(Value::as_str)
                .ok_or_else(|| AppError::CloudUnavailable("missing upload_url".into()))?;
            let method = presign
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("PUT");
            let mut request = self.http.request(
                Method::from_bytes(method.as_bytes()).unwrap_or(Method::PUT),
                upload_url,
            );
            if let Some(headers) = presign.get("headers").and_then(Value::as_object) {
                for (key, value) in headers {
                    if let Some(value) = value.as_str() {
                        request = request.header(key, value);
                    }
                }
            }
            let response = request
                .body(bytes)
                .send()
                .await
                .map_err(|error| AppError::Network(error.to_string()))?;
            if !response.status().is_success() {
                return Err(AppError::CloudUnavailable(format!(
                    "object storage upload failed with {}",
                    response.status()
                )));
            }
        }

        self.cloud
            .authorized_json(
                Method::POST,
                &format!("/api/v1/files/{file_id}/complete"),
                Some(&json!({})),
            )
            .await?;
        sqlx::query(
            "UPDATE source_revisions SET cloud_file_id = ?, cloud_object_key = ? WHERE id = ?",
        )
        .bind(file_id)
        .bind(&object_key)
        .bind(row.id)
        .execute(&self.pool)
        .await?;
        self.upsert_file_state(
            row.id,
            &row.content_hash,
            Some(file_id),
            Some(&object_key),
            "uploaded",
            None,
        )
        .await?;
        Ok(())
    }

    async fn download_missing(&self) -> Result<(usize, Vec<String>), AppError> {
        let rows = self.pending_downloads().await?;
        let mut downloaded = 0;
        let mut reindex = Vec::new();
        for row in rows {
            let relative_path = BlobStore::relative_path_for_hash(&row.content_hash);
            let valid_local = match self.blobs.get_bytes(&relative_path).await {
                Ok(bytes) => {
                    bytes.len() as i64 == row.size_bytes && sha256_hex(&bytes) == row.content_hash
                }
                Err(_) => false,
            };
            if valid_local {
                self.set_local_blob_path(row.id, &relative_path).await?;
                self.upsert_file_state(
                    row.id,
                    &row.content_hash,
                    row.cloud_file_id.as_deref(),
                    None,
                    "downloaded",
                    None,
                )
                .await?;
                reindex.push(row.source_id.to_string());
                continue;
            }
            match self.download_one(&row, &relative_path).await {
                Ok(()) => {
                    downloaded += 1;
                    reindex.push(row.source_id.to_string());
                }
                Err(error) => {
                    self.mark_failed(row.id, &error.to_string()).await;
                    tracing::warn!(revision = %row.id, %error, "cloud file download failed");
                }
            }
        }
        Ok((downloaded, reindex))
    }

    async fn download_one(&self, row: &RevisionRow, relative_path: &str) -> Result<(), AppError> {
        let file_id = row
            .cloud_file_id
            .as_deref()
            .ok_or_else(|| AppError::CloudUnavailable("missing cloud file id".into()))?;
        let body = json!({ "file_id": file_id });
        let response = self
            .cloud
            .authorized_json(Method::POST, "/api/v1/files/presign-download", Some(&body))
            .await?;
        let presign: Value = response
            .json()
            .await
            .map_err(|error| AppError::CloudUnavailable(error.to_string()))?;
        let download_url = presign
            .get("download_url")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::CloudUnavailable("missing download_url".into()))?;
        let mut response = self
            .http
            .get(download_url)
            .send()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        if response.status() != StatusCode::OK {
            return Err(AppError::CloudUnavailable(format!(
                "object storage download failed with {}",
                response.status()
            )));
        }
        if row.size_bytes <= 0 || row.size_bytes > 50 * 1024 * 1024 {
            return Err(AppError::InvalidInput(
                "cloud file exceeds import limit".into(),
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?
        {
            if chunk.len() > (row.size_bytes as usize).saturating_sub(bytes.len()) {
                return Err(AppError::InvalidInput(
                    "cloud file exceeds declared size".into(),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.len() as i64 != row.size_bytes {
            return Err(AppError::InvalidInput("cloud file size mismatch".into()));
        }
        let actual_hash = sha256_hex(&bytes);
        if actual_hash != row.content_hash {
            return Err(AppError::CloudSyncConflict(format!(
                "downloaded file hash mismatch for revision {}",
                row.id
            )));
        }
        self.blobs.put_bytes(&bytes).await?;
        self.set_local_blob_path(row.id, relative_path).await?;
        self.upsert_file_state(
            row.id,
            &row.content_hash,
            Some(file_id),
            None,
            "downloaded",
            None,
        )
        .await?;
        Ok(())
    }

    async fn pending_uploads(&self) -> Result<Vec<RevisionRow>, AppError> {
        let rows = sqlx::query(
            "SELECT r.id, r.source_id, r.content_hash, r.blob_path, r.mime_type, r.size_bytes, \
                    s.original_filename, r.cloud_file_id \
             FROM source_revisions r \
             JOIN sources s ON s.id = r.source_id \
             LEFT JOIN cloud_file_sync f ON f.source_revision_id = r.id \
             WHERE r.cloud_file_id IS NULL \
               AND (f.status IS NULL OR f.status IN ('pending', 'failed')) \
             ORDER BY r.created_at ASC LIMIT 50",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(row_to_revision).collect()
    }

    async fn pending_downloads(&self) -> Result<Vec<RevisionRow>, AppError> {
        let rows = sqlx::query(
            "SELECT r.id, r.source_id, r.content_hash, r.blob_path, r.mime_type, r.size_bytes, \
                    s.original_filename, r.cloud_file_id \
             FROM source_revisions r \
             JOIN sources s ON s.id = r.source_id \
             LEFT JOIN cloud_file_sync f ON f.source_revision_id = r.id \
             WHERE r.cloud_file_id IS NOT NULL \
               AND (f.status IS NULL OR f.status IN ('pending', 'failed') OR s.status != 'ready' OR NOT EXISTS (SELECT 1 FROM documents d WHERE d.source_id = s.id) OR EXISTS (SELECT 1 FROM source_index_state i WHERE i.source_id = s.id AND i.status != 'ready')) \
             ORDER BY r.created_at ASC LIMIT 50",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(row_to_revision).collect()
    }

    async fn set_local_blob_path(
        &self,
        revision_id: Uuid,
        relative_path: &str,
    ) -> Result<(), AppError> {
        // This path is device-local, so keep it out of the sync journal.
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE sync_apply_guard SET active = 1 WHERE id = 1")
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE source_revisions SET blob_path = ? WHERE id = ?")
            .bind(relative_path)
            .bind(revision_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE sync_apply_guard SET active = 0 WHERE id = 1")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn upsert_file_state(
        &self,
        revision_id: Uuid,
        content_hash: &str,
        file_id: Option<&str>,
        object_key: Option<&str>,
        status: &str,
        last_error: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO cloud_file_sync (source_revision_id, content_hash, file_id, object_key, status, last_error, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(source_revision_id) DO UPDATE SET \
                 content_hash = excluded.content_hash, \
                 file_id = excluded.file_id, \
                 object_key = excluded.object_key, \
                 status = excluded.status, \
                 last_error = excluded.last_error, \
                 updated_at = excluded.updated_at",
        )
        .bind(revision_id)
        .bind(content_hash)
        .bind(file_id)
        .bind(object_key)
        .bind(status)
        .bind(last_error)
        .bind(utc_now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn mark_failed(&self, revision_id: Uuid, error: &str) {
        let _ = sqlx::query(
            "INSERT INTO cloud_file_sync (source_revision_id, content_hash, status, last_error, updated_at) \
             SELECT id, content_hash, 'failed', ?, ? FROM source_revisions WHERE id = ? \
             ON CONFLICT(source_revision_id) DO UPDATE SET status = 'failed', last_error = excluded.last_error, updated_at = excluded.updated_at",
        )
        .bind(error)
        .bind(utc_now().to_rfc3339())
        .bind(revision_id)
        .execute(&self.pool)
        .await;
    }
}

fn row_to_revision(row: sqlx::sqlite::SqliteRow) -> Result<RevisionRow, AppError> {
    Ok(RevisionRow {
        id: row.try_get("id")?,
        source_id: row.try_get("source_id")?,
        content_hash: row.try_get("content_hash")?,
        blob_path: row.try_get("blob_path")?,
        mime_type: row.try_get("mime_type")?,
        size_bytes: row.try_get("size_bytes")?,
        filename: row.try_get("original_filename")?,
        cloud_file_id: row.try_get("cloud_file_id")?,
    })
}
