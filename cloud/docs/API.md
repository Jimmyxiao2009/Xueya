# Verruckt Cloud API

Base URL: `http://localhost:8080`

Authenticated endpoints expect:

```http
Authorization: Bearer <access_token>
```

Errors:

```json
{
  "error": {
    "code": "invalid_credentials",
    "message": "invalid email or password",
    "retryable": false,
    "request_id": "9f7c..."
  }
}
```

Every response includes `x-request-id`.

## Health

### `GET /healthz`

```json
{"status":"ok","version":"0.1.0"}
```

### `GET /readyz`

```json
{
  "status": "ready",
  "database": "ok",
  "migrations": "ok",
  "config": "ok",
  "providers": {"ai": "not_configured"}
}
```

## Auth

### `POST /api/v1/auth/register`

```json
{
  "email": "user@example.com",
  "password": "correct horse battery staple",
  "device": {
    "name": "My Laptop",
    "platform": "windows",
    "app_version": "0.1.0"
  }
}
```

Response `201`:

```json
{
  "access_token": "...",
  "refresh_token": "...",
  "token_type": "Bearer",
  "expires_in": 1800,
  "user": {
    "id": "uuid",
    "email": "user@example.com",
    "display_name": null,
    "created_at": "2026-01-01T00:00:00Z"
  },
  "device": {
    "id": "uuid",
    "name": "My Laptop",
    "platform": "windows",
    "app_version": "0.1.0",
    "created_at": "2026-01-01T00:00:00Z",
    "last_seen_at": "2026-01-01T00:00:00Z"
  }
}
```

Duplicate email: `409 email_already_registered`.

### `POST /api/v1/auth/login`

```json
{
  "email": "user@example.com",
  "password": "correct horse battery staple",
  "device_id": "optional-existing-device-uuid",
  "device": {
    "name": "My Laptop",
    "platform": "windows",
    "app_version": "0.1.0"
  }
}
```

Same response shape as register. Invalid credentials always return
`401 invalid_credentials`. Device creation can fail with
`403 device_limit_reached`.

### `POST /api/v1/auth/refresh`

```json
{"refresh_token":"..."}
```

Same response shape as login/register. The old refresh token is revoked and
replaced. Reusing a rotated token returns
`401 refresh_token_reuse_detected` and revokes the session family.

### `POST /api/v1/auth/logout`

```json
{"refresh_token":"optional"}
```

If `refresh_token` is supplied, its family is revoked. Otherwise the current
access-token session is revoked. Returns `204`.

## Me and entitlements

### `GET /api/v1/me`

```json
{
  "user": {
    "id": "uuid",
    "email": "user@example.com",
    "display_name": null,
    "created_at": "2026-01-01T00:00:00Z"
  }
}
```

### `GET /api/v1/me/entitlements`

```json
{
  "plan": "pro",
  "features": {"cloud_sync": true, "official_ai": true},
  "limits": {
    "max_devices": 5,
    "max_cloud_storage_bytes": 5368709120,
    "monthly_ai_credits": 100000
  },
  "usage": {
    "devices_used": 2,
    "monthly_ai_credits_used": 1234,
    "cloud_storage_bytes_used": 456789
  }
}
```

## Devices

### `GET /api/v1/me/devices`

Returns the current user's devices.

### `DELETE /api/v1/me/devices/{id}`

Returns `204` or `404 device_not_found`.

## Sync

All sync endpoints require the `cloud_sync` entitlement.

### `POST /api/v1/sync/push`

```json
{
  "device_id": "uuid",
  "changes": [
    {
      "change_id": "uuid",
      "entity_type": "KnowledgeNode",
      "entity_id": "uuid",
      "base_revision": 0,
      "revision": 1,
      "operation": "upsert",
      "updated_at": "2026-01-01T00:00:00Z",
      "payload": {"opaque": "client-owned json"}
    }
  ]
}
```

Response:

```json
{
  "accepted": [
    {
      "change_id": "uuid",
      "entity_type": "KnowledgeNode",
      "entity_id": "uuid",
      "revision": 1,
      "already_applied": false
    }
  ],
  "conflicts": [],
  "new_cursor": "AAAAAAAAAAE"
}
```

Retrying the same `change_id` returns `already_applied: true`.

Conflict example:

```json
{
  "change_id": "uuid",
  "entity_type": "KnowledgeNode",
  "entity_id": "uuid",
  "server_version": 19,
  "client_version": 18,
  "server_payload": {"opaque": "server json"},
  "server_updated_at": "2026-01-01T00:00:00Z",
  "deleted": false
}
```

### `GET /api/v1/sync/pull?cursor=...&limit=100`

`cursor` is opaque and monotonic. `limit` is clamped to 1..500.

```json
{
  "changes": [
    {
      "cursor": "AAAAAAAAAAE",
      "device_id": "uuid",
      "change_id": "uuid",
      "entity_type": "KnowledgeNode",
      "entity_id": "uuid",
      "revision": 1,
      "operation": "upsert",
      "payload": {},
      "created_at": "2026-01-01T00:00:00Z"
    }
  ],
  "next_cursor": "AAAAAAAAAAE",
  "has_more": false
}
```

### `GET /api/v1/sync/bootstrap?cursor=...&limit=200`

```json
{
  "entities": [
    {
      "entity_type": "KnowledgeNode",
      "entity_id": "uuid",
      "revision": 2,
      "deleted": false,
      "payload": {},
      "updated_at": "2026-01-01T00:00:00Z"
    }
  ],
  "next_cursor": "AAAAAAAAAAI",
  "has_more": false,
  "sync_cursor": "AAAAAAAAAAE"
}
```

### `POST /api/v1/sync/ack`

```json
{"device_id":"uuid","cursor":"AAAAAAAAAAE"}
```

```json
{"acknowledged_cursor":"AAAAAAAAAAE"}
```

## Files

### `POST /api/v1/files/presign-upload`

```json
{
  "sha256": "64 lowercase or uppercase hex characters",
  "size": 12345,
  "mime": "application/pdf",
  "filename": "optional-original-name.pdf"
}
```

```json
{
  "file_id": "uuid",
  "object_key": "users/uuid/objects/sha256",
  "upload_url": "http://minio:9000/verruckt-cloud/users/...?X-Amz-Signature=...",
  "method": "PUT",
  "headers": {"Content-Type": "application/pdf"},
  "expires_in": 900,
  "deduplicated": false
}
```

Checks before signing: `cloud_sync` entitlement, storage limit, per-user
SHA-256 dedup.

### `POST /api/v1/files/presign-download`

```json
{"file_id":"uuid"}
```

```json
{
  "file_id": "uuid",
  "object_key": "users/uuid/objects/sha256",
  "download_url": "http://minio:9000/...?X-Amz-Signature=...",
  "method": "GET",
  "expires_in": 900
}
```

Non-owner gets `404 file_not_found`.

### `DELETE /api/v1/files/{id}`

```json
{"deleted":true,"file_id":"uuid"}
```

## Official AI gateway

All AI endpoints require the `official_ai` entitlement and an account access
token. Use logical model names only.

Logical models: `verruckt-fast`, `verruckt-standard`, `verruckt-reasoning`,
`verruckt-embedding`.

### `POST /v1/chat/completions`

OpenAI-compatible subset:

```json
{
  "model": "verruckt-fast",
  "messages": [{"role": "user", "content": "hello"}],
  "max_tokens": 256,
  "stream": false
}
```

Non-streaming response is the upstream JSON with `model` replaced by the
logical model. `usage` is preserved and recorded.

Streaming returns `text/event-stream` with upstream SSE chunks relayed as they
arrive. The server requests `stream_options.include_usage` and commits actual
usage at the end.

### `POST /api/v1/ai/chat/stream`

Same body as chat completions; `stream` is forced to `true`.

### `POST /v1/embeddings` and `POST /api/v1/ai/embeddings`

```json
{
  "model": "verruckt-embedding",
  "input": ["text one", "text two"]
}
```

Validated limits: body size, chat message count, max tokens, embedding input
count, embedding total characters.

### `GET /api/v1/ai/usage`

```json
{
  "period_start": "2026-01-01T00:00:00Z",
  "credits_used": 1234,
  "credits_limit": 100000,
  "credits_remaining": 98766,
  "events": [
    {
      "id": "uuid",
      "operation": "chat",
      "logical_model": "verruckt-fast",
      "upstream_model": "gpt-4o-mini",
      "input_tokens": 12,
      "output_tokens": 34,
      "estimated_cost_units": 46,
      "status": "completed",
      "request_id": "uuid",
      "latency_ms": 812,
      "created_at": "2026-01-01T00:00:00Z"
    }
  ]
}
```

Quota exhaustion returns `402 ai_quota_exceeded`. Upstream provider failures are
mapped to safe error codes without leaking upstream credentials.
