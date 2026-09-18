use super::adapters::{adapter_for, composite_id, SCHEMA_VERSION};
use crate::cloud::{CloudAccountClient, CloudAccountState};
use crate::error::AppError;
use crate::util::time::utc_now;
use reqwest::Method;
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

const PULL_LIMIT: i64 = 100;
const MAX_PAGES: usize = 50;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatusDto {
    pub logged_in: bool,
    pub sync_enabled: bool,
    pub cloud_sync_entitled: bool,
    pub bootstrap_done: bool,
    pub pending_changes: i64,
    pub open_conflicts: i64,
    pub cursor: String,
    pub last_push_at: Option<String>,
    pub last_pull_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummary {
    pub pushed: usize,
    pub pulled: usize,
    pub conflicts: usize,
    pub skipped: bool,
    pub reason: Option<String>,
}

impl SyncSummary {
    fn skipped(reason: &str) -> Self {
        Self {
            pushed: 0,
            pulled: 0,
            conflicts: 0,
            skipped: true,
            reason: Some(reason.to_owned()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflictDto {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub server_revision: i64,
    pub client_revision: i64,
    pub created_at: String,
}

/// Local-first delta sync.
///
/// Every method is safe to call from a background trigger: if the user is not
/// signed in, does not have the cloud_sync entitlement, or has the sync toggle
/// off, the service reports "skipped" and touches nothing.
pub struct CloudSyncService {
    cloud: Arc<CloudAccountClient>,
    pool: SqlitePool,
    lock: Arc<AsyncMutex<()>>,
}

impl CloudSyncService {
    pub fn new(cloud: Arc<CloudAccountClient>, pool: SqlitePool) -> Self {
        Self {
            cloud,
            pool,
            lock: Arc::new(AsyncMutex::new(())),
        }
    }

    pub async fn status(&self) -> Result<SyncStatusDto, AppError> {
        let account = self.cloud.account_state().await?;
        let row = sqlx::query(
            "SELECT cursor, bootstrap_done, last_push_at, last_pull_at, last_error FROM sync_state WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        let (cursor, bootstrap_done, last_push_at, last_pull_at, last_error) = match row {
            Some(row) => (
                row.try_get::<String, _>("cursor")?,
                row.try_get::<i64, _>("bootstrap_done")? != 0,
                row.try_get::<Option<String>, _>("last_push_at")?,
                row.try_get::<Option<String>, _>("last_pull_at")?,
                row.try_get::<Option<String>, _>("last_error")?,
            ),
            None => (String::new(), false, None, None, None),
        };
        let pending_changes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_outbox")
            .fetch_one(&self.pool)
            .await?;
        let open_conflicts: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sync_conflicts WHERE resolved = 0")
                .fetch_one(&self.pool)
                .await?;
        Ok(SyncStatusDto {
            logged_in: account.logged_in,
            sync_enabled: account.sync_enabled,
            cloud_sync_entitled: account
                .entitlements
                .as_ref()
                .map(|entitlements| entitlements.cloud_sync)
                .unwrap_or(false),
            bootstrap_done,
            pending_changes,
            open_conflicts,
            cursor,
            last_push_at,
            last_pull_at,
            last_error,
        })
    }

    pub async fn conflicts(&self) -> Result<Vec<SyncConflictDto>, AppError> {
        let rows = sqlx::query(
            "SELECT id, entity_type, entity_id, server_revision, client_revision, created_at \
             FROM sync_conflicts WHERE resolved = 0 ORDER BY created_at DESC LIMIT 100",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut conflicts = Vec::with_capacity(rows.len());
        for row in rows {
            conflicts.push(SyncConflictDto {
                id: row.try_get("id")?,
                entity_type: row.try_get("entity_type")?,
                entity_id: row.try_get("entity_id")?,
                server_revision: row.try_get("server_revision")?,
                client_revision: row.try_get("client_revision")?,
                created_at: row.try_get("created_at")?,
            });
        }
        Ok(conflicts)
    }

    /// Push local changes, then pull remote changes. Never blocks learning:
    /// callers may ignore the error and the outbox remains durable.
    pub async fn sync_now(&self) -> Result<SyncSummary, AppError> {
        let account = self.cloud.account_state().await?;
        if !account.logged_in {
            return Ok(SyncSummary::skipped("not_signed_in"));
        }
        if !account.sync_enabled {
            return Ok(SyncSummary::skipped("sync_disabled"));
        }
        if !account
            .entitlements
            .as_ref()
            .map(|entitlements| entitlements.cloud_sync)
            .unwrap_or(false)
        {
            return Ok(SyncSummary::skipped("entitlement_required"));
        }

        let _guard = self.lock.lock().await;
        let result = self.run_once(&account).await;
        if let Err(error) = &result {
            self.record_error(&error.to_string()).await;
        }
        result
    }

    async fn run_once(&self, account: &CloudAccountState) -> Result<SyncSummary, AppError> {
        let device_id = self.require_device_id().await?;
        let bootstrapped = self.ensure_bootstrap(account, device_id).await?;
        let (pushed, conflicts) = self.push_outbox(device_id).await?;
        let pulled = self.pull_changes(device_id).await?;
        self.mark_synced().await?;
        Ok(SyncSummary {
            pushed,
            pulled,
            conflicts,
            skipped: false,
            reason: if bootstrapped {
                Some("bootstrapped".to_owned())
            } else {
                None
            },
        })
    }

    async fn require_device_id(&self) -> Result<Uuid, AppError> {
        self.cloud
            .device_id()
            .await?
            .ok_or(AppError::CloudAuth("cloud device is not registered".into()))
    }

    async fn ensure_bootstrap(
        &self,
        account: &CloudAccountState,
        device_id: Uuid,
    ) -> Result<bool, AppError> {
        let bootstrap_done: i64 =
            sqlx::query_scalar("SELECT bootstrap_done FROM sync_state WHERE id = 1")
                .fetch_optional(&self.pool)
                .await?
                .unwrap_or(0);
        if bootstrap_done != 0 {
            return Ok(false);
        }
        if !account
            .entitlements
            .as_ref()
            .map(|entitlements| entitlements.cloud_sync)
            .unwrap_or(false)
        {
            return Ok(false);
        }

        let mut cursor = self.cursor().await?;
        for _ in 0..MAX_PAGES {
            let path = format!(
                "/api/v1/sync/bootstrap?limit={PULL_LIMIT}&cursor={}",
                encode_query(&cursor)
            );
            let response = self.cloud.authorized_json(Method::GET, &path, None).await?;
            let body: Value = response
                .json()
                .await
                .map_err(|error| AppError::CloudUnavailable(error.to_string()))?;
            let entities = body
                .get("entities")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let applied = self.apply_remote_entities(&entities).await?;
            let next_cursor = body
                .get("next_cursor")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let has_more = body
                .get("has_more")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            cursor = next_cursor;
            if !has_more {
                let sync_cursor = body
                    .get("sync_cursor")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| cursor.clone());
                self.finish_bootstrap(&sync_cursor).await?;
                let _ = self.ack(device_id, &sync_cursor).await;
                tracing::debug!(applied, "cloud sync bootstrap applied entities");
                return Ok(true);
            }
        }
        Err(AppError::CloudUnavailable(
            "cloud sync bootstrap did not finish".into(),
        ))
    }

    async fn apply_remote_entities(&self, entities: &[Value]) -> Result<usize, AppError> {
        if entities.is_empty() {
            return Ok(0);
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE sync_apply_guard SET active = 1 WHERE id = 1")
            .execute(&mut *tx)
            .await?;
        let mut ordered: Vec<&Value> = entities.iter().collect();
        ordered.sort_by_key(|entity| {
            entity_priority(
                entity
                    .get("entity_type")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
        });
        let mut applied = 0;
        for entity in ordered {
            let entity_type = entity
                .get("entity_type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let sync_id = entity
                .get("entity_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok());
            let revision = entity.get("revision").and_then(Value::as_i64).unwrap_or(0);
            let deleted = entity
                .get("deleted")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let payload = entity.get("payload").cloned().unwrap_or(Value::Null);
            let Some(sync_id) = sync_id else { continue };
            let Some(adapter) = adapter_for(entity_type) else {
                tracing::debug!(entity_type, "ignoring unknown cloud sync entity");
                continue;
            };
            let Some(local_id) = resolve_local_id(entity_type, sync_id, Some(&payload)) else {
                continue;
            };
            if has_open_conflict(&mut tx, entity_type, &local_id).await? {
                continue;
            }
            let local_revision = local_revision(&mut tx, entity_type, &local_id).await?;
            if local_revision > 0 && local_revision != revision {
                record_conflict_in_tx(
                    &mut tx,
                    entity_type,
                    &local_id,
                    revision,
                    local_revision,
                    &payload,
                    deleted,
                )
                .await?;
                continue;
            }
            if deleted {
                adapter.delete(&mut tx, &local_id).await?;
            } else {
                adapter.apply(&mut tx, &local_id, &payload).await?;
                remember_entity_id(&mut tx, entity_type, sync_id, &local_id).await?;
            }
            set_local_revision(&mut tx, entity_type, &local_id, revision).await?;
            applied += 1;
        }
        sqlx::query("UPDATE sync_apply_guard SET active = 0 WHERE id = 1")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(applied)
    }

    async fn push_outbox(&self, device_id: Uuid) -> Result<(usize, usize), AppError> {
        let rows = sqlx::query(
            "SELECT change_id, entity_type, entity_id, operation, base_revision, revision \
             FROM sync_outbox ORDER BY CASE entity_type \
                WHEN 'course' THEN 0 WHEN 'knowledge_point' THEN 1 WHEN 'course_node' THEN 2 \
                WHEN 'course_node_dependency' THEN 3 WHEN 'node_knowledge_point' THEN 4 \
                WHEN 'lesson_step' THEN 5 WHEN 'learner_state' THEN 6 WHEN 'quiz_question' THEN 7 \
                WHEN 'quiz_attempt' THEN 8 WHEN 'adaptation_event' THEN 9 \
                WHEN 'knowledge_object' THEN 10 WHEN 'note' THEN 11 \
                WHEN 'knowledge_relation' THEN 12 WHEN 'source' THEN 13 \
                WHEN 'source_revision' THEN 14 ELSE 99 END, created_at ASC LIMIT 200",
        )
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Ok((0, 0));
        }

        let mut changes = Vec::new();
        // (normalized uuid string, raw journal id)
        let mut change_ids: Vec<(String, String)> = Vec::new();
        for row in &rows {
            let change_id: String = row.try_get("change_id")?;
            let change_key = Uuid::parse_str(&change_id)
                .map(|uuid| uuid.to_string())
                .unwrap_or_else(|_| change_id.clone());
            let entity_type: String = row.try_get("entity_type")?;
            let local_id: String = row.try_get("entity_id")?;
            let operation: String = row.try_get("operation")?;
            let base_revision: i64 = row.try_get("base_revision")?;
            let revision: i64 = row.try_get("revision")?;
            let Some(adapter) = adapter_for(&entity_type) else {
                // An unknown local journal entry can never be pushed; drop it
                // so it cannot wedge the queue forever.
                self.remove_outbox(&change_id).await?;
                continue;
            };
            let payload = if operation == "delete" {
                json!({ "schema_version": SCHEMA_VERSION })
            } else {
                match adapter.load(&self.pool, &local_id).await? {
                    Some(mut payload) => {
                        payload["schema_version"] = json!(SCHEMA_VERSION);
                        payload
                    }
                    None => continue,
                }
            };
            let sync_id = sync_id_for(&entity_type, &local_id);
            remember_entity_id_pool(&self.pool, &entity_type, sync_id, &local_id).await?;
            changes.push(json!({
                "change_id": change_id,
                "entity_type": entity_type,
                "entity_id": sync_id,
                "base_revision": base_revision,
                "revision": revision,
                "operation": operation,
                "updated_at": utc_now().to_rfc3339(),
                "payload": payload,
            }));
            change_ids.push((change_key, change_id));
        }

        if changes.is_empty() {
            return Ok((0, 0));
        }

        let body = json!({ "device_id": device_id, "changes": changes });
        let response = self
            .cloud
            .authorized_json(Method::POST, "/api/v1/sync/push", Some(&body))
            .await?;
        let response_body: Value = response
            .json()
            .await
            .map_err(|error| AppError::CloudUnavailable(error.to_string()))?;

        let mut accepted = 0usize;
        if let Some(entries) = response_body.get("accepted").and_then(Value::as_array) {
            for entry in entries {
                let Some(change_id) = entry.get("change_id").and_then(Value::as_str) else {
                    continue;
                };
                let change_key = Uuid::parse_str(change_id)
                    .map(|uuid| uuid.to_string())
                    .unwrap_or_else(|_| change_id.to_owned());
                let Some((_, raw_change_id)) =
                    change_ids.iter().find(|(key, _)| key == &change_key)
                else {
                    continue;
                };
                let entity_type = entry
                    .get("entity_type")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let entity_id = entry
                    .get("entity_id")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok());
                let revision = entry.get("revision").and_then(Value::as_i64).unwrap_or(0);
                self.remove_outbox(raw_change_id).await?;
                if let Some(entity_id) = entity_id {
                    if let Some(local_id) = self.lookup_local_id(entity_type, entity_id).await? {
                        set_local_revision_pool(&self.pool, entity_type, &local_id, revision)
                            .await?;
                    }
                }
                accepted += 1;
            }
        }

        let mut conflicts = 0usize;
        if let Some(entries) = response_body.get("conflicts").and_then(Value::as_array) {
            for entry in entries {
                let Some(change_id) = entry.get("change_id").and_then(Value::as_str) else {
                    continue;
                };
                let change_key = Uuid::parse_str(change_id)
                    .map(|uuid| uuid.to_string())
                    .unwrap_or_else(|_| change_id.to_owned());
                let Some((_, raw_change_id)) =
                    change_ids.iter().find(|(key, _)| key == &change_key)
                else {
                    continue;
                };
                let entity_type = entry
                    .get("entity_type")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let entity_id = entry
                    .get("entity_id")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok());
                let server_revision = entry
                    .get("server_version")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let client_revision = entry
                    .get("client_version")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let server_payload = entry.get("server_payload").cloned().unwrap_or(Value::Null);
                let deleted = entry
                    .get("deleted")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.remove_outbox(raw_change_id).await?;
                if let Some(entity_id) = entity_id {
                    if let Some(local_id) = self.lookup_local_id(entity_type, entity_id).await? {
                        self.record_conflict(
                            entity_type,
                            &local_id,
                            server_revision,
                            client_revision,
                            &server_payload,
                            deleted,
                        )
                        .await?;
                        // Move the base forward so the next local edit is compared
                        // against the server version, not the stale one.
                        set_local_revision_pool(
                            &self.pool,
                            entity_type,
                            &local_id,
                            server_revision,
                        )
                        .await?;
                    }
                }
                conflicts += 1;
            }
        }

        self.touch_push_time().await?;
        Ok((accepted, conflicts))
    }

    async fn pull_changes(&self, device_id: Uuid) -> Result<usize, AppError> {
        let mut cursor = self.cursor().await?;
        let mut pulled = 0usize;
        for _ in 0..MAX_PAGES {
            let path = format!(
                "/api/v1/sync/pull?limit={PULL_LIMIT}&cursor={}",
                encode_query(&cursor)
            );
            let response = self.cloud.authorized_json(Method::GET, &path, None).await?;
            let body: Value = response
                .json()
                .await
                .map_err(|error| AppError::CloudUnavailable(error.to_string()))?;
            let changes = body
                .get("changes")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            pulled += self.apply_pull_changes(&changes).await?;
            let next_cursor = body
                .get("next_cursor")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let has_more = body
                .get("has_more")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            cursor = next_cursor;
            if !has_more {
                break;
            }
        }
        self.set_cursor(&cursor).await?;
        self.touch_pull_time().await?;
        let _ = self.ack(device_id, &cursor).await;
        Ok(pulled)
    }

    async fn apply_pull_changes(&self, changes: &[Value]) -> Result<usize, AppError> {
        if changes.is_empty() {
            return Ok(0);
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE sync_apply_guard SET active = 1 WHERE id = 1")
            .execute(&mut *tx)
            .await?;
        let mut ordered: Vec<&Value> = changes.iter().collect();
        ordered.sort_by_key(|change| {
            entity_priority(
                change
                    .get("entity_type")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
        });
        let mut applied = 0;
        for change in ordered {
            let entity_type = change
                .get("entity_type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let sync_id = change
                .get("entity_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok());
            let operation = change
                .get("operation")
                .and_then(Value::as_str)
                .unwrap_or("upsert");
            let revision = change.get("revision").and_then(Value::as_i64).unwrap_or(0);
            let payload = change.get("payload").cloned().unwrap_or(Value::Null);
            let Some(sync_id) = sync_id else { continue };
            let Some(adapter) = adapter_for(entity_type) else {
                continue;
            };
            let local_id = if operation == "delete" {
                match lookup_local_id(&mut tx, entity_type, sync_id).await? {
                    Some(local_id) => local_id,
                    None => continue,
                }
            } else {
                match resolve_local_id(entity_type, sync_id, Some(&payload)) {
                    Some(local_id) => local_id,
                    None => continue,
                }
            };
            if has_open_conflict(&mut tx, entity_type, &local_id).await? {
                continue;
            }
            if operation == "delete" {
                adapter.delete(&mut tx, &local_id).await?;
            } else {
                match adapter.apply(&mut tx, &local_id, &payload).await {
                    Ok(()) => {
                        remember_entity_id(&mut tx, entity_type, sync_id, &local_id).await?;
                    }
                    Err(AppError::CloudSyncConflict(reason)) => {
                        record_conflict_in_tx(
                            &mut tx,
                            entity_type,
                            &local_id,
                            revision,
                            revision,
                            &payload,
                            false,
                        )
                        .await?;
                        tracing::warn!(%reason, entity_type, "skipping unsupported sync payload");
                        continue;
                    }
                    Err(error) => return Err(error),
                }
            }
            set_local_revision(&mut tx, entity_type, &local_id, revision).await?;
            applied += 1;
        }
        sqlx::query("UPDATE sync_apply_guard SET active = 0 WHERE id = 1")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(applied)
    }

    async fn cursor(&self) -> Result<String, AppError> {
        Ok(
            sqlx::query_scalar("SELECT cursor FROM sync_state WHERE id = 1")
                .fetch_optional(&self.pool)
                .await?
                .unwrap_or_default(),
        )
    }

    async fn set_cursor(&self, cursor: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE sync_state SET cursor = ? WHERE id = 1")
            .bind(cursor)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn finish_bootstrap(&self, cursor: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE sync_state SET bootstrap_done = 1, cursor = ? WHERE id = 1")
            .bind(cursor)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn mark_synced(&self) -> Result<(), AppError> {
        sqlx::query("UPDATE sync_state SET last_error = NULL WHERE id = 1")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn record_error(&self, message: &str) {
        let _ = sqlx::query("UPDATE sync_state SET last_error = ? WHERE id = 1")
            .bind(message)
            .execute(&self.pool)
            .await;
    }

    async fn touch_push_time(&self) -> Result<(), AppError> {
        sqlx::query("UPDATE sync_state SET last_push_at = ? WHERE id = 1")
            .bind(utc_now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn touch_pull_time(&self) -> Result<(), AppError> {
        sqlx::query("UPDATE sync_state SET last_pull_at = ? WHERE id = 1")
            .bind(utc_now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn ack(&self, device_id: Uuid, cursor: &str) -> Result<(), AppError> {
        let body = json!({ "device_id": device_id, "cursor": cursor });
        let _ = self
            .cloud
            .authorized_json(Method::POST, "/api/v1/sync/ack", Some(&body))
            .await?;
        Ok(())
    }

    async fn remove_outbox(&self, change_id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM sync_outbox WHERE change_id = ?")
            .bind(change_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn lookup_local_id(
        &self,
        entity_type: &str,
        sync_id: Uuid,
    ) -> Result<Option<String>, AppError> {
        let mut connection = self.pool.acquire().await?;
        lookup_local_id(&mut connection, entity_type, sync_id).await
    }

    async fn record_conflict(
        &self,
        entity_type: &str,
        entity_id: &str,
        server_revision: i64,
        client_revision: i64,
        server_payload: &Value,
        deleted: bool,
    ) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await?;
        record_conflict_in_tx(
            &mut tx,
            entity_type,
            entity_id,
            server_revision,
            client_revision,
            server_payload,
            deleted,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

/// Foreign-key-safe apply order. Server snapshots and pull pages can arrive in
/// any order; a child row must never be inserted before its parent.
fn entity_priority(entity_type: &str) -> u8 {
    match entity_type {
        "course" => 0,
        "knowledge_point" => 1,
        "course_node" => 2,
        "course_node_dependency" => 3,
        "node_knowledge_point" => 4,
        "lesson_step" => 5,
        "learner_state" => 6,
        "quiz_question" => 7,
        "quiz_attempt" => 8,
        "adaptation_event" => 9,
        "knowledge_object" => 10,
        "note" => 11,
        "knowledge_relation" => 12,
        "source" => 13,
        "source_revision" => 14,
        _ => 99,
    }
}

fn sync_id_for(entity_type: &str, local_id: &str) -> Uuid {
    if let Ok(uuid) = Uuid::parse_str(local_id) {
        return uuid;
    }
    Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        format!("verruckt:{entity_type}:{local_id}").as_bytes(),
    )
}

fn resolve_local_id(entity_type: &str, sync_id: Uuid, payload: Option<&Value>) -> Option<String> {
    let payload = payload?;
    let string = |key: &str| payload.get(key).and_then(Value::as_str);
    match entity_type {
        "course_node_dependency" => Some(composite_id(
            string("node_id")?,
            string("depends_on_node_id")?,
        )),
        "node_knowledge_point" => Some(composite_id(
            string("node_id")?,
            string("knowledge_point_id")?,
        )),
        "learner_state" => Some(composite_id(
            string("course_id")?,
            string("knowledge_point_id")?,
        )),
        _ => Some(hex::encode(sync_id.as_bytes())),
    }
}

async fn lookup_local_id(
    connection: &mut sqlx::SqliteConnection,
    entity_type: &str,
    sync_id: Uuid,
) -> Result<Option<String>, AppError> {
    let local_id: Option<String> = sqlx::query_scalar(
        "SELECT local_id FROM sync_entity_ids WHERE entity_type = ? AND sync_id = ?",
    )
    .bind(entity_type)
    .bind(sync_id.to_string())
    .fetch_optional(&mut *connection)
    .await?;
    Ok(local_id)
}

async fn remember_entity_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    entity_type: &str,
    sync_id: Uuid,
    local_id: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO sync_entity_ids (entity_type, sync_id, local_id) VALUES (?, ?, ?) \
         ON CONFLICT(entity_type, sync_id) DO UPDATE SET local_id = excluded.local_id",
    )
    .bind(entity_type)
    .bind(sync_id.to_string())
    .bind(local_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn remember_entity_id_pool(
    pool: &SqlitePool,
    entity_type: &str,
    sync_id: Uuid,
    local_id: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO sync_entity_ids (entity_type, sync_id, local_id) VALUES (?, ?, ?) \
         ON CONFLICT(entity_type, sync_id) DO UPDATE SET local_id = excluded.local_id",
    )
    .bind(entity_type)
    .bind(sync_id.to_string())
    .bind(local_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn has_open_conflict(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    entity_type: &str,
    entity_id: &str,
) -> Result<bool, AppError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sync_conflicts \
         WHERE resolved = 0 AND entity_type = ? AND entity_id = ? COLLATE NOCASE",
    )
    .bind(entity_type)
    .bind(entity_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(count > 0)
}

async fn local_revision(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    entity_type: &str,
    entity_id: &str,
) -> Result<i64, AppError> {
    Ok(sqlx::query_scalar(
        "SELECT revision FROM sync_entity_meta WHERE entity_type = ? AND entity_id = ? COLLATE NOCASE",
    )
    .bind(entity_type)
    .bind(entity_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or(0))
}

async fn set_local_revision(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    entity_type: &str,
    entity_id: &str,
    revision: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at) \
         VALUES (?, ?, ?, ?) ON CONFLICT(entity_type, entity_id) DO UPDATE SET \
         revision = excluded.revision, updated_at = excluded.updated_at",
    )
    .bind(entity_type)
    .bind(entity_id)
    .bind(revision)
    .bind(utc_now().to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn set_local_revision_pool(
    pool: &SqlitePool,
    entity_type: &str,
    entity_id: &str,
    revision: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at) \
         VALUES (?, ?, ?, ?) ON CONFLICT(entity_type, entity_id) DO UPDATE SET \
         revision = excluded.revision, updated_at = excluded.updated_at",
    )
    .bind(entity_type)
    .bind(entity_id)
    .bind(revision)
    .bind(utc_now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

async fn record_conflict_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    entity_type: &str,
    entity_id: &str,
    server_revision: i64,
    client_revision: i64,
    server_payload: &Value,
    deleted: bool,
) -> Result<(), AppError> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO sync_conflicts (id, entity_type, entity_id, server_revision, client_revision, \
         server_payload_json, deleted, resolved, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?)",
    )
    .bind(id)
    .bind(entity_type)
    .bind(entity_id)
    .bind(server_revision)
    .bind(client_revision)
    .bind(server_payload.to_string())
    .bind(if deleted { 1_i64 } else { 0_i64 })
    .bind(utc_now().to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn encode_query(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
