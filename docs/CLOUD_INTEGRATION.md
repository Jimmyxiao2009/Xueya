# Verruckt Cloud integration

This document is the operator's guide to the optional Cloud features in the
desktop app. The product rule is **Local First, Cloud Optional, BYOK
First-class, Official AI Optional, Sync Optional**. A Cloud failure must never
destroy the ability to learn.

---

## 1. What the Cloud owns

Verruckt Cloud is a control plane, not a learning engine. It owns:

* accounts, rotating refresh tokens and device registration;
* memberships, plan entitlements, usage counters and storage quota;
* a delta-sync relay for authoritative local state;
* presigned object-storage upload/download for source files;
* an AI gateway that maps logical model names to upstream OpenAI-compatible
  providers and enforces quota.

It does **not** own curriculum, learner state, retrieval, adaptation or any
other learning business logic. Those stay in the local SQLite database.

The desktop integration surface is small:

```
src-tauri/src/cloud/client.rs    CloudAccountClient
src-tauri/src/cloud/commands.rs  cloud_* Tauri commands
src-tauri/src/cloud/models.rs    account / entitlement / device DTOs
src-tauri/src/ai/cloud.rs        CloudAiProvider (Chat + Embeddings)
src-tauri/src/sync/engine.rs     CloudSyncService
src-tauri/src/sync/files.rs      CloudFileSyncService
src/stores/cloudStore.ts         frontend state and friendly errors
src/components/settings/CloudAccountSection.tsx
```

---

## 2. Session and secrets

* The access token is kept **in memory only**.
* The refresh token is stored in the OS keyring under
  `secret://cloud/refresh-token`.
* The frontend can ask for account state, entitlements and devices, but there
  is no command that returns either token.
* Every authenticated call goes through `CloudAccountClient::authorized_json`.
  On a 401 it performs a single-flight refresh behind a `tokio::sync::Mutex`,
  retries the original request exactly once, and on failure clears the session
  and emits `cloud_signed_out`.
* Only auth failures clear the session. A transient `CLOUD_UNAVAILABLE` or
  network error leaves the refresh token in the keyring, so offline learning
  continues and a later refresh can recover.

Cloud base URL defaults to `VERRUCKT_CLOUD_URL` or
`http://127.0.0.1:8080`. It can be changed in Settings; the path must not end
in `/v1` because the client adds `/v1` exactly once.

---

## 3. Capability model

The desktop consumes an `Entitlements` shape:

```ts
interface CloudEntitlements {
  plan: string;
  features: {
    cloud_sync: boolean;
    official_ai: boolean;
    // unknown features are preserved
  };
  limits: {
    max_devices: number;
    max_cloud_storage_bytes: number;
    monthly_ai_credits: number;
  };
  usage: {
    devices_used: number;
    monthly_ai_credits_used: number;
    cloud_storage_bytes_used: number;
  };
}
```

Cloud AI is offered only when `official_ai` is true. Sync is offered only when
`cloud_sync` is true. The AI source radio in Settings is:

1. **Verruckt Cloud** — official AI, gated by entitlement and quota;
2. **自定义模型服务** — the existing BYOK provider;
3. **离线模式** — the deterministic mock.

`AiMode` defaults to `byok` when a real BYOK provider exists, otherwise
`offline`. Cloud falls back to BYOK/mock when it cannot serve, so the UI never
loses the ability to answer.

---

## 4. Cloud AI protocol

Cloud AI is **Chat Completions only** from the desktop:

* `POST /v1/chat/completions`
* `POST /v1/embeddings`

Logical models:

| Logical model | Use |
| --- | --- |
| `verruckt-fast` | default chat |
| `verruckt-standard` | balanced chat |
| `verruckt-reasoning` | harder reasoning |
| `verruckt-embedding` | embeddings |

The Cloud gateway maps these to `VERRUCKT_UPSTREAM_*_MODEL`. The desktop never
assumes an embedding dimension: it probes the returned vector and stores the
observed dimension in the local index metadata.

Streaming uses SSE deltas and the existing `AiStreamEvent` lifecycle
(Started / Delta / Completed / Error). The frontend batches deltas on a short
timer / animation frame and unsubscribes when the learner leaves the screen.

---

## 5. Local-first delta sync

### Journal

Authoritative writes are captured transactionally by SQLite triggers in
`src-tauri/migrations/0004_cloud_sync.sql`:

* `sync_outbox` — one row per entity, with `base_revision` and `revision`;
* `sync_entity_meta` — the last server revision known locally;
* `sync_apply_guard` — suppresses outbox writes while a pull is being applied;
* `sync_conflicts` — unresolved conflicts, with the server payload;
* `sync_entity_ids` — maps composite local ids to deterministic UUIDv5 ids.

Payloads carry `schema_version = 1`. Only authoritative state is synced:
courses, nodes, knowledge points, lesson steps, learner states, quiz
questions/attempts, adaptation events, notes, knowledge objects/relations,
sources and source revisions. Chunks, embeddings and FTS indexes are rebuilt
locally.

### Cursors and crash safety

`CloudSyncService` performs:

1. `ensure_bootstrap()` — paginated `GET /api/v1/sync/bootstrap`;
2. `push_outbox()` — `POST /api/v1/sync/push`;
3. `pull_changes()` — paginated `GET /api/v1/sync/pull`;
4. `ack()` — `POST /api/v1/sync/ack`.

The outbox row is removed only after the server accepts or reports a conflict,
so a crash between push and ack cannot lose a local write.

### Conflict policy

If the server revision is not the local base revision, the push is reported as
a conflict. The engine:

* records the conflict in `sync_conflicts`;
* leaves the local row untouched;
* moves the local base revision forward so the next local edit is compared
  against the current server version;
* skips later pulls for that entity while the conflict is open.

No silent overwrite.

### Triggers

Sync runs:

* once on startup;
* after a short debounce when local writes happen;
* on a periodic timer;
* manually from Settings;
* on the frontend foreground event when available.

Cloud failure is non-fatal: the outbox remains durable and learning continues.

---

## 6. Source files

Source files are synced through object storage, not the delta payload:

1. Desktop requests `POST /api/v1/files/presign-upload`;
2. Desktop PUTs the bytes directly to MinIO/S3;
3. the revision row records `cloud_file_id` / `cloud_object_key`;
4. another device requests `POST /api/v1/files/presign-download`;
5. the device GETs the object, verifies SHA-256, stores it in the local
   content-addressed blob store, and schedules a local re-index.

Embeddings are never transferred. The receiving device rebuilds chunks and
embeddings locally. Identical bytes are deduplicated by SHA-256.

---

## 7. Error mapping

Cloud errors use stable codes and map to friendly Chinese copy in
`src/lib/cloudErrors.ts`:

| Cloud code | Desktop `AppError` | UI behaviour |
| --- | --- | --- |
| `invalid_credentials` | `CLOUD_AUTH_ERROR` | wrong email/password |
| `token_expired` / `access_token_expired` | `CLOUD_SESSION_EXPIRED` | clear session, sign in again |
| `entitlement_required` | `CLOUD_ENTITLEMENT_REQUIRED` | feature locked |
| `ai_quota_exceeded` | `CLOUD_QUOTA_EXCEEDED` | quota copy + usage |
| `storage_quota_exceeded` | `CLOUD_STORAGE_QUOTA_EXCEEDED` | storage copy |
| `sync_conflict` | `CLOUD_SYNC_CONFLICT` | conflict warning |
| `device_limit_reached` | `CLOUD_ENTITLEMENT_REQUIRED` | remove a device |
| `upstream_*` / `internal_error` | `CLOUD_UNAVAILABLE` | retryable banner |

`AppError` redacts anything that looks like a secret before serialising or
logging it.

---

## 8. Docker Compose integration environment

`cloud/docker-compose.yml` starts:

* `postgres:16-alpine` on `5432`;
* `minio` on `9000` (console `9001`);
* `mock-openai` on `8099` (the OpenAI-compatible upstream);
* `verruckt-cloud` on `8080`.

The mock upstream supports `POST /v1/chat/completions` (JSON and SSE) and
`POST /v1/embeddings`, with usage fields so quota paths can be exercised.

Run the real desktop → Cloud → mock upstream test with:

```powershell
pwsh -File scripts/e2e-cloud.ps1
```

The script starts Compose, creates a fresh account, upgrades it to `pro`,
and runs:

```powershell
cargo test --test cloud_live -- --ignored --nocapture --test-threads=1
```

`cloud_live` covers login/entitlements, Cloud chat, Cloud embeddings, delta
sync A→B, conflict preservation, source file upload/download with SHA-256
verification, local re-index, and session persistence across a restart.

---

## 9. Known gaps

* A real WebView smoke against Cloud is not automated; the E2E drives the Rust
  client and provider layer directly.
* Conflict resolution preserves local data and warns, but there is no
  side-by-side choose-server/merge UI yet.
* The live Cloud test is opt-in and needs Docker; it is not part of the default
  `cargo test`.
* Quota is enforced by the Cloud gateway and cached for display; the desktop
  does not pre-reserve credits before an AI call.
