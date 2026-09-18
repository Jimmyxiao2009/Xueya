# Orbit Backend API (Tauri IPC)

Every command the WebView can call, its input, its output, and the error
contract. This is the reference for `src/api.ts`; if the two disagree, the Rust
source in `src-tauri/src/ipc/` wins and this file is stale.

---

## 1. Calling convention

* Transport: Tauri 2 `invoke(command, args)`.
* `command`: `snake_case`, always.
* `args`: a single object. Scalar arguments are named after the Rust parameter
  (`{ id }`, `{ nodeId }`); structured arguments are wrapped in `{ input }`.
* Output: the command's return value, `camelCase` keys (Rust structs carry
  `#[serde(rename_all = "camelCase")]`).
* `src/api.ts` is the **only** module that calls `invoke`. Screens use `api.*`.

### Why `{ input }`

Commands that take a struct take `input: XInput` rather than flattening fields
onto the argument object. It keeps the Rust signature honest (one deserialised
struct) and makes adding an optional field a non-breaking change on both sides.

---

## 2. Error contract

Any command can fail. Rejections are always an object:

```ts
interface AppErrorShape {
  code: string;          // stable, machine-readable
  message: string;       // safe to show a user
  details?: string | null; // redacted diagnostic, or null
  retryable: boolean;
}
```

`src/api.ts::errorOf()` normalises anything else (a bare string, a thrown
non-object) into `{ code: "INTERNAL_ERROR", … }`, so `catch` blocks always get
the same shape.

### Codes

| Code | Meaning | `retryable` |
| --- | --- | --- |
| `DATABASE_ERROR` | SQLite failure | yes |
| `IO_ERROR` | file / keyring failure | no |
| `INVALID_INPUT` | validation failed | no |
| `NOT_FOUND` | unknown id | no |
| `PROVIDER_ERROR` | AI provider returned an error | yes |
| `PROVIDER_AUTH_ERROR` | bad or missing API key | no |
| `NETWORK_ERROR` | transport failure | yes |
| `SEARCH_ERROR` | web search provider failure | yes |
| `PARSER_ERROR` | document could not be parsed | no |
| `EMBEDDING_ERROR` | embedding request failed | yes |
| `AI_RESPONSE_INVALID` | model output failed schema validation | no |
| `JOB_CANCELLED` | job cancelled by the user | no |
| `UNSUPPORTED_FORMAT` | file type not supported | no |
| `INTERNAL_ERROR` | anything else | no |

### Secret redaction

`AppError::public_message()` returns a fixed, safe sentence per variant — raw
provider text is **never** surfaced. `details()` only includes the raw message
for `INVALID_INPUT`, `NOT_FOUND` and `UNSUPPORTED_FORMAT`, and even then it runs
through `redact()`, which strips anything shaped like a key, bearer token or
`sk-…` string. `PROVIDER_AUTH_ERROR` deliberately carries no detail at all.

---

## 3. Commands

### App & settings

| Command | Args | Returns |
| --- | --- | --- |
| `app_bootstrap` | — | `AppInfo` — runs migrations, seeds the demo routes, then returns |
| `app_get_info` | — | `AppInfo` |
| `settings_get` | — | `{ searchProvider: string; hasBraveApiKey: boolean }` |
| `settings_update` | `{ input: { searchProvider?: string; braveApiKey?: string } }` | same as `settings_get` |

```ts
interface AppInfo {
  version: string;
  dataDir: string;
  providerCount: number;
  sourceCount: number;
  courseCount: number;
  searchConfigured: boolean;
}
```

`app_bootstrap` is idempotent: the two demo routes (射影几何, 线性代数) are only
inserted when `courses.seeded` is empty, so they exist on first launch and are
never re-created after the user deletes them.

### Providers (BYOK)

| Command | Args | Returns |
| --- | --- | --- |
| `provider_list` | — | `Provider[]` |
| `provider_upsert` | `{ input: ProviderInput }` | `Provider` |
| `provider_delete` | `{ id }` | `void` |
| `provider_test` | `{ id }` | `{ ok, message, protocol?, capabilities? }` |
| `provider_stream_chat` | `{ input: { providerId, prompt, requestId? } }` | `void` (events) |

```ts
interface Provider {
  id: string;
  displayName: string;
  baseUrl: string;
  chatModel: string;
  embeddingModel: string;
  enabled: boolean;
  hasApiKey: boolean;   // <- the key itself is never returned
  protocol: "auto" | "responses" | "chat_completions";
  maskedHint?: string | null;
}

interface ProviderCapabilities {
  chatCompletions: boolean;
  responses: boolean;
  streaming: boolean;
  structuredOutput: boolean;
  embeddings: boolean;
  embeddingDimension?: number | null;
  visionOptional: boolean;
  protocol?: string | null;
  probeErrors: string[];
}
```

`apiKey` is accepted on `provider_upsert`, written to the OS keyring
(`secrets/keyring_store.rs`), and **never read back over IPC**. There is no
command that returns a key.

`provider_stream_chat` emits `provider_stream` events shaped as
`{ requestId, event }`, where `event` is one of `started`, `delta`,
`citationMetadata`, `completed`, `failed`. The event never contains a key.

### Sources

| Command | Args | Returns |
| --- | --- | --- |
| `source_list` | — | `Source[]` |
| `source_import_bytes` | `{ input: { filename, bytesBase64, title?, mimeType? } }` | `Job` |
| `source_import_files` | `{ input: { paths: string[], title? } }` | `Job[]` |
| `source_import_url` | `{ input: { url, title? } }` | `Job` |
| `source_delete` | `{ id }` | `void` |
| `source_reindex` | `{ id }` | `Job` |
| `source_summarize` | `{ id }` | `SourceSummary` |

Import commands return immediately with a `Job`. When the job completes,
`resultJson` is `{"sourceId":"<id>"}` — that is how the frontend learns the new
source's id (`sourceStore.readSourceId`).

### Search & knowledge

| Command | Args | Returns |
| --- | --- | --- |
| `search_web` | `{ input: { query, count? } }` | `{ title, url, snippet, source }[]` |
| `knowledge_search` | `{ input: { query, limit? } }` | `KnowledgeHit[]` |
| `knowledge_get_object` | `{ id, path? }` | `{ id, kind, title, description? }` |
| `knowledge_get_related` | `{ id, depth? }` | `KnowledgeHit[]` |
| `knowledge_retrieval_debug` | `{ input: { query, routeId?, sourceId?, knowledgePointId?, limit? } }` | `RetrievalHit[]` (dev-only) |

`knowledge_search` works with **no configuration**: with no provider key the
query and the documents are embedded by the same deterministic `MockProvider`
space, so ranking and citations are real, just not semantic. `knowledge_search`
uses the hybrid retriever (FTS5 + vector); `knowledge_retrieval_debug` exposes
the component scores for the dev inspector.

### Routes (backend: "courses")

| Command | Args | Returns |
| --- | --- | --- |
| `course_list` | — | `LearningRoute[]` |
| `course_get` | `{ id }` | `RouteDetail` |
| `course_create` | `{ input: RouteCreateInput }` | `Job` |
| `course_delete` | `{ id }` | `void` |
| `course_regenerate` | `{ id }` | `Job` |
| `course_node_get` | `{ nodeId }` | `RouteNode` |
| `course_node_prepare` | `{ nodeId }` | `Lesson` |
| `course_node_complete` | `{ nodeId }` | `CourseNodeComplete` |
| `course_learner_states` | `{ id }` | `LearnerState[]` |
| `course_adaptations` | `{ id, limit? }` | `AdaptationEvent[]` |

```ts
interface RouteCreateInput {
  topic: string;
  goal?: string | null;
  experienceLevel?: "beginner" | "some_experience" | "advanced" | null;
  desiredDepth?: "overview" | "practical" | "deep" | null;
  sourceIds?: string[];
}
```

`course_create` returns a `Job`; on completion its `resultJson` is
`{"courseId":"<id>"}`.

`course_node_prepare` is safe to call repeatedly — it caches the generated
lesson per node and returns the same content.

`course_node_complete` marks the node completed, unlocks the next one, and
returns the refreshed course. The `adaptation` field is populated when finishing
the node also changed the roadmap.

### Learning

| Command | Args | Returns |
| --- | --- | --- |
| `learning_start` | `{ input: { nodeId } }` | `{ sessionId, lesson, mastery }` |
| `learning_respond` | `{ input: { sessionId, answer } }` | `LearningEvaluation` |
| `learning_finish` | `{ input: { sessionId } }` | `void` |
| `quiz_list` | `{ nodeId }` | `QuizQuestion[]` |
| `quiz_submit` | `{ input: { questionId, answer: string[], sessionId? } }` | `QuizResult` |
| `learning_respond` | `{ input: { sessionId, answer } }` | `LearningEvaluation` |
| `tutor_send` | `{ input: { sessionId, message } }` | `TutorTurn` |
| `tutor_send_stream` | `{ input: { sessionId, message, requestId? } }` | `TutorTurn` (events) |

`learning_respond` is the model's door for answers string matching cannot judge
(`short`). It requires an active session and a non-empty answer, writes a
`learning_attempts` row, and returns correctness, misconceptions, a mastery
delta and a follow-up question. Imported evidence is used when present but is
no longer required.

`tutor_send` persists both sides of the conversation into `learning_messages`.
If the reply names a misconception, it also inserts a `misconception_detected`
adaptation and splices a `加练 · …` review node after the current node — the
same node a failed check question would have created, never a second one.
`TutorTurn.adaptation` carries the roadmap change (kind `review_inserted`) so
the chat can say what happened.

`tutor_send_stream` runs the same retrieval/citation/adaptation pipeline, but
consumes the provider SSE stream and emits `tutor_stream` events shaped as
`{ requestId, event }` (`started`, `delta`, `citationMetadata`, `completed`,
`failed`). The returned `TutorTurn` is authoritative, so the caller replaces
the buffered text with it after the stream ends. The streamed prompt uses
inline `[C1]` citations and an optional final `DIAGNOSIS:` line; Rust strips
that line and maps labels to real chunks before persisting.

```ts
interface QuizResult {
  questionId: string;
  nodeId: string;
  correct: boolean;
  graded: boolean;         // false for `short` / `image`
  score: number;
  feedback: string;
  expected: string[];      // revealed only after a wrong answer
  misconceptions: string[];
  mastery: number;
  confidence: number;
  nextReview?: string | null;
  adaptation?: AdaptationEvent | null;  // set when the roadmap changed
  insertedNodeId?: string | null;
}
```

#### Answer payloads

| Question kind | Send | Graded |
| --- | --- | --- |
| `single` | `["<option text>"]` | yes |
| `multi` | `["<option text>", …]` (any order) | yes |
| `fill` | `["<raw text>"]` | yes, case/whitespace-insensitive; containment accepted for answers ≥ 2 chars |
| `short` | `["<raw text>"]` | no → `已提交 · 待评阅` |
| `image` | `[]` | no → `已提交 · 待评阅` |

Option **texts**, not indices. `domain/quiz.rs::expected()` resolves stored
indices to their option text so both sides agree.

Ungraded kinds deliberately do not write `learner_states`, do not emit an
adaptation, and do not block advancing — the UI treats `graded: false` as
"submitted", never as "wrong".

### Jobs

| Command | Args | Returns |
| --- | --- | --- |
| `job_list` | — | `Job[]` |
| `job_get` | `{ id }` | `Job` |
| `job_cancel` | `{ id }` | `Job` |
| `job_subscribe` | `{ id }` | streams `job://progress` events |

```ts
interface Job {
  id: string;
  kind: string;         // generate_route | source_ingestion | reindex | ...
  status: string;       // pending | running | completed | failed | cancelled
  progress: number;     // 0..1
  message: string;      // human-readable phase, shown verbatim
  errorJson?: string | null;
  resultJson?: string | null;
  createdAt: string;
  startedAt?: string | null;
  completedAt?: string | null;
}
```

The frontend uses `watchJob(id, onUpdate)` (`src/api.ts`): it polls `job_get`
every 220 ms until the status is one of `completed | failed | cancelled`, and
calls `onUpdate` with each intermediate job so the generation screen can show
real progress.

---

## 4. Typed DTOs

`src/api.ts` declares an interface for every payload above. Nothing in the app
consumes an untyped JSON value except the two `*Json` string fields
(`metadataJson`, `payloadJson`), which are parsed by
`src/lib/nodeMeta.ts::routeMeta` / `nodeMeta`.

### Metadata keys

Written by Rust, read by the frontend, **camelCase**:

| Field | Keys |
| --- | --- |
| `courses.metadata_json` | `learningObjectives`, `prerequisites`, `sourceIds`, `evidenceChunkIds`, `origin` |
| `course_nodes.metadata_json` | `concepts`, `dependencies`, `evidenceChunkIds`, `knowledgePointId`, `origin`, `insertedFor`, `reason` |

---

## 5. Adding a command

1. `src-tauri/src/domain/` — the types and any pure rules.
2. `src-tauri/src/db/repo/` — the SQL, with a `#[cfg(test)]` round-trip test.
3. `src-tauri/src/services/` — orchestration; no SQL here.
4. `src-tauri/src/ipc/<area>.rs` — the `#[tauri::command]` plus its DTO, with
   `#[serde(rename_all = "camelCase")]`.
5. `src-tauri/src/lib.rs` — add it to `generate_handler!`.
6. `src/api.ts` — add the DTO interface and the `api.*` wrapper.
7. This document, and `src-tauri/tests/learning_loop.rs` if it is part of the
   learning loop.
