# Verruckt Cloud

Verruckt Cloud is a lightweight **control plane** for Verruckt / Orbit. It is not
the learning engine. Local Core keeps SQLite learning data, curriculum, learner
model, adaptation, source parsing, chunking, embedding indexes, retrieval, RAG,
tutor context, and prompt construction.

Cloud responsibilities:

1. Account / authentication
2. Membership / entitlements
3. Device management
4. Cross-device entity-level delta sync
5. Source file object-storage coordination
6. Official AI gateway
7. AI usage / quota
8. Docker deployment

## Workspace

```text
cloud/
  Cargo.toml
  crates/verruckt-cloud/          # HTTP server + admin CLI
  crates/verruckt-cloud-domain/   # passwords, tokens, entitlements, sync revisions
  crates/verruckt-cloud-protocol/ # request/response DTOs
  crates/verruckt-cloud-storage/  # S3-compatible presign/delete abstraction
  migrations/
  Dockerfile
  docker-compose.yml
  .env.example
  docs/API.md
```

The repository root workspace is intentionally not modified.

## Local vs Cloud

| Concern | Local Core | Cloud |
| --- | --- | --- |
| SQLite learning data | yes | no |
| Curriculum / routes / lessons | yes | no |
| Learner model / adaptation | yes | no |
| Source parsing / chunking / embeddings / RAG | yes | no |
| Tutor prompt construction | yes | no |
| Accounts, sessions, devices | no | yes |
| Plans, subscriptions, entitlements | no | yes |
| Delta sync relay | client merge | change journal / snapshot relay |
| Source file bytes | local read/write | coordinates S3-compatible storage |
| Official AI calls | optional BYOK | logical-model gateway |

## Authentication

`POST /api/v1/auth/register`, `/login`, `/refresh`, `/logout`.

- Argon2id password hashing with random salt.
- Login failures return one `invalid_credentials` error so account existence is
  not leaked.
- Access tokens are short-lived HS256 JWTs (default 30 minutes).
- Refresh tokens are 256-bit random URL-safe strings. Only SHA-256 hashes are
  stored in PostgreSQL.
- Refresh tokens rotate. Reusing a rotated token revokes the entire session
  family.

## Entitlements

- `plans`, `subscriptions`, `entitlements` (feature flags + numeric limits),
  `usage_counters`.
- `free`: no cloud sync, no official AI, 1 device, 0 AI credits, 0 storage.
- `pro`: cloud sync, official AI, 5 devices, 100k monthly AI credits, 5 GiB
  storage.
- `GET /api/v1/me/entitlements` returns `plan`, `features`, `limits`, `usage`.
- Payment providers are not integrated in this round; grant Pro with
  `cloud-admin set-plan`.

## Sync

Cloud never syncs the SQLite file. It relays entity-level changes.

- `POST /api/v1/sync/push`
- `GET /api/v1/sync/pull?cursor=...&limit=...`
- `GET /api/v1/sync/bootstrap?cursor=...&limit=...`
- `POST /api/v1/sync/ack`

Properties:

- `sync_changes.cursor` is a durable `BIGSERIAL`, not a timestamp.
- Pull cursors are opaque base64url encodings of the durable sequence.
- Push uses revision conflict detection and returns `server_version`,
  `client_version`, `server_payload`, and `server_updated_at` on conflict.
- `change_id` is idempotent per user. Retrying a push does not duplicate journal
  rows.
- Delete is a tombstone (`deleted = true`).
- Every query is scoped by `user_id`.
- Bootstrap pages authoritative snapshots and returns the current sync cursor.

## Object storage

Source file bytes never travel through sync JSON.

- `POST /api/v1/files/presign-upload`
- `POST /api/v1/files/presign-download`
- `DELETE /api/v1/files/:id`

Cloud checks the `cloud_sync` entitlement and storage limit before signing.
Object keys are `users/{user_id}/objects/{sha256}`; filenames are never keys.
Files are deduplicated per user by SHA-256. Cloud does not parse, OCR, embed, or
index source files.

## Official AI gateway

The client constructs AI requests. Cloud handles authentication, entitlement,
quota, rate limiting, provider routing, streaming relay, and usage accounting.

Logical models: `verruckt-fast`, `verruckt-standard`, `verruckt-reasoning`,
`verruckt-embedding`.

OpenAI-compatible subset: `POST /v1/chat/completions`,
`POST /v1/embeddings`.

Native aliases: `POST /api/v1/ai/chat`, `POST /api/v1/ai/chat/stream`,
`POST /api/v1/ai/embeddings`, `GET /api/v1/ai/usage`.

Streaming relays upstream SSE bytes as they arrive. Cloud does not persist
prompts, chat bodies, retrieved chunks, or source plaintext.

## Quota and rate limiting

- AI credits are reserved before the upstream call and committed after usage is
  known.
- Reservations are transactionally checked against committed usage and active
  reservations.
- Failed or dropped streams release the reservation.
- Exhaustion returns `402 ai_quota_exceeded` before upstream cost is incurred.
- Rate limiting is per-user and in-process in this round. Multiple instances
  need a shared limiter such as Redis before horizontal scaling.

## Security

- All sync, file, and AI endpoints require an account access token.
- Every repository query is scoped by `user_id`.
- Guessing a file or device UUID does not reveal another user's data.
- Uniform error envelope with `code`, `message`, `retryable`, `request_id`.
- SQL errors, backtraces, upstream secrets, and internal URLs are never returned
  to clients.
- Upstream API keys are read only from server environment/secret files.
- CORS is deny-by-default. Set `VERRUCKT_CORS_ORIGINS` explicitly.
- Body limits, request timeouts, and upstream timeouts are configured.

## Environment

Core: `VERRUCKT_ENV`, `VERRUCKT_BIND`, `VERRUCKT_PORT`, `DATABASE_URL`,
`VERRUCKT_ACCESS_TOKEN_SECRET`, `VERRUCKT_ACCESS_TOKEN_TTL_SECS`,
`VERRUCKT_REFRESH_TOKEN_TTL_DAYS`, `VERRUCKT_TOKEN_ISSUER`,
`VERRUCKT_CORS_ORIGINS`, `VERRUCKT_MAX_BODY_BYTES`,
`VERRUCKT_REQUEST_TIMEOUT_SECS`, `VERRUCKT_LOG_FORMAT`.

Object storage: `VERRUCKT_OBJECT_STORAGE_ENDPOINT`, `_REGION`, `_BUCKET`,
`_ACCESS_KEY`, `_SECRET_KEY`, `_FORCE_PATH_STYLE`, `_PRESIGN_TTL_SECS`.

Upstream AI: `VERRUCKT_UPSTREAM_BASE_URL`, `_API_KEY`, `_CHAT_MODEL`,
`_REASONING_MODEL`, `_EMBEDDING_MODEL`, `VERRUCKT_MODEL_FAST`,
`VERRUCKT_MODEL_STANDARD`, `VERRUCKT_MODEL_REASONING`,
`VERRUCKT_MODEL_EMBEDDING`, `VERRUCKT_AI_RATE_LIMIT_PER_MINUTE`,
`VERRUCKT_AI_REQUEST_TIMEOUT_SECS`, `VERRUCKT_AI_MAX_MESSAGES`,
`VERRUCKT_AI_MAX_TOKENS`, `VERRUCKT_AI_MAX_EMBEDDING_INPUTS`,
`VERRUCKT_AI_MAX_EMBEDDING_TOTAL_CHARS`.

Secret file variants are supported by appending `_FILE`, for example
`VERRUCKT_OBJECT_STORAGE_SECRET_KEY_FILE`.

## Docker

```bash
cd cloud
cp .env.example .env
docker compose up -d
curl http://localhost:8080/healthz
curl http://localhost:8080/readyz
```

Development stack: `verruckt-cloud`, `postgres`, `minio`. PostgreSQL and MinIO
data use named volumes and survive container restarts.

Production notes: use the provided multi-stage Dockerfile; run as the non-root
`verruckt` user; use a real secret manager or `*_FILE` secrets; terminate TLS in
front of the service; set `VERRUCKT_CORS_ORIGINS` only to trusted web origins;
use an external shared limiter before running multiple instances; back up
PostgreSQL regularly and follow the object store durability policy.

## Admin CLI

```bash
cloud-admin create-user --email user@example.com --password '...' --plan pro
cloud-admin set-plan --email user@example.com --plan pro
cloud-admin show-user --email user@example.com
cloud-admin reset-usage --email user@example.com
```

## Verification

```bash
cd cloud
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
docker compose build
docker compose up -d
```

Real PostgreSQL integration tests:

```bash
VERRUCKT_TEST_DATABASE_URL=postgres://verruckt:verruckt@localhost:5432/verruckt \
  cargo test --workspace -- --nocapture
```

Optional MinIO-backed file tests:

```bash
VERRUCKT_TEST_OBJECT_STORAGE_ENDPOINT=http://localhost:9000 \
VERRUCKT_TEST_OBJECT_STORAGE_BUCKET=verruckt-cloud \
VERRUCKT_TEST_OBJECT_STORAGE_ACCESS_KEY=minioadmin \
VERRUCKT_TEST_OBJECT_STORAGE_SECRET_KEY=minioadmin \
  cargo test --workspace -- --nocapture
```
