# Orbit / Verruckt — Architecture

Orbit is a local-first desktop learning app. It turns sources into a living
learning route and keeps the whole loop — **Sources → Synthesis → Roadmap →
Learn → Question → Diagnose → Adapt → Review** — inside one SQLite database on
the user's machine.

This document describes the shape of the system as it is built today. Anything
marked _not wired_ is genuinely absent, not "coming soon" wording for work that
was silently skipped.

---

## 1. Runtime shape

```
┌──────────────────────────────── WebView (React 19) ────────────────────────────────┐
│                                                                                    │
│  screens/            components/                  lib/                              │
│  ├ CreateRouteScreen ├ layout/  AppHeader, Toast  ├ swipeEngine.ts   (imperative)    │
│  ├ GeneratingScreen  ├ roadmap/ RoadmapScreen,    ├ lessonTransition.ts             │
│  └ ...               │          RoadmapLayer,     ├ roadPath.ts                     │
│                      │          NoticeBanner      ├ nodeMeta.ts / format.ts / icons │
│                      ├ lesson/  LessonScreen,     └ ...                             │
│                      │          LessonPane                                          │
│                      └ learning/ LearningScreen, LearningPlan, QuizCard             │
│                                                                                    │
│  stores/ (zustand)  uiStore · routeStore · jobStore · lessonStore · learningStore   │
│                     sourceStore                                                     │
│                                                                                    │
│  api.ts  ── typed wrappers + DTOs, the ONLY place that calls invoke() ──────────────┤
└────────────────────────────────────────┬───────────────────────────────────────────┘
                                         │ Tauri IPC (invoke / commands)
┌────────────────────────────────────────┴───────────────────────────────────────────┐
│                              Rust core (`verruckt_lib`)                            │
│                                                                                    │
│  ipc/        thin command layer: parse DTO → call service → serialise DTO           │
│  services/   use-case orchestration (course, learning, quiz, tutor, ingestion…)     │
│  domain/     pure types + rules (course, plan, quiz, learner, adaptation, job…)     │
│  db/repo/    sqlx queries, one module per aggregate                                 │
│  ai/         AIProvider trait, MockProvider, OpenAI-compatible provider, registry    │
│  curriculum/ deterministic planner: topic + depth + level → node list               │
│  ingest/     file/URL → text → chunks → embeddings                                  │
│  knowledge/  retrieval, rerank, vector index, graph                                 │
│  jobs/       background job manager, progress events, cancellation                  │
│  secrets/    API keys in the OS keyring; never returned to the WebView              │
└────────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                  SQLite (app data dir)
```

### Why the split

* **`api.ts` is the only IPC caller.** Every command has one typed wrapper and
  one DTO interface. Screens never see a raw JSON blob, and a backend rename
  breaks the build instead of failing at runtime.
* **Rust owns the truth.** The frontend never computes progress, mastery or
  adaptation — it renders what `course_get` / `quiz_submit` return.
* **`ai/` is a trait boundary.** `ChatProvider` and `EmbeddingProvider` are
  traits; `MockProvider` implements both. With no API key, retrieval, lesson
  synthesis and quiz generation still work, deterministically.

---

The window is the app's only chrome, so its geometry is a preference:
`tauri-plugin-window-state` writes size, position and maximised state to
`<appConfigDir>/.window-state.json` on a graceful close and restores it on
launch. This was verified by seeding a size, launching, and reading the real
window rect back through `GetWindowRect`.

## 2. Three layers of the product

| Layer | Question it answers | Where it lives |
| --- | --- | --- |
| **Research** | "What do my sources actually say?" | `ingest/`, `knowledge/`, `knowledge_fs/`, `search/` |
| **Curriculum** | "What is the order, and why?" | `curriculum/`, `services/course_service.rs`, `domain/plan.rs` |
| **Learning** | "What does *this* learner do next?" | `services/learning_service.rs`, `learning/`, `domain/learner.rs`, `domain/adaptation.rs` |

The Research layer produces evidence chunks. The Curriculum layer turns them
into nodes. The Learning layer mutates the route in response to answers — that
mutation is what makes Orbit different from a static course list.

---

## 3. Data model

`src-tauri/migrations/0001_init.sql` holds the base schema (sources, documents,
chunks, embeddings, courses, course_nodes, jobs, providers, settings).
`0002_learning_loop.sql` adds the learning loop:

| Table | Purpose |
| --- | --- |
| `courses` (+`goal`, `experience_level`, `desired_depth`, `kind`, `current_node_id`, `seeded`) | a route; `seeded` marks the two demo routes |
| `course_nodes` (+`kind`, `confidence`, `review_at`, `generated_by`) | one stop on the roadmap; `kind ∈ lesson·review·quiz·practice` |
| `knowledge_points` / `node_knowledge_points` | what a node teaches, and the mastery attached to it |
| `lesson_steps` | the "study plan" list shown inside a lesson |
| `learner_states` | per knowledge point: mastery, confidence, attempts, review time |
| `quiz_questions` / `quiz_attempts` | generated questions and every submission |
| `adaptation_events` | an append-only log of *why* the roadmap changed |
| `learning_messages` | the tutor conversation attached to a session |
| `jobs.result_json` | job artefacts (e.g. `{"sourceId": "…"}`, `{"courseId": "…"}`) |

Node and course metadata are stored as JSON strings with **camelCase** keys
(`concepts`, `dependencies`, `evidenceChunkIds`, `knowledgePointId`, `origin`,
`learningObjectives`, `sourceIds`, …) so the frontend can read them without a
mapping layer.

### Naming: `Course` vs `LearningRoute`

The backend calls a route a **course**; the UI calls it a **route** (学习路线).
This is a deliberate, documented mapping rather than a 15-file rename:

| Backend | Frontend |
| --- | --- |
| `Course` / `course_list` / `course_get` | `LearningRoute` / `api.routeList()` / `api.routeGet()` |
| `CourseNode` / `course_node_*` | `RouteNode` / `api.nodePrepare()` … |

The wire DTOs use the backend names; `src/api.ts` is where the rename happens.

---

## 4. The learning loop, end to end

0. **Collect** — if 联网搜集资料 is on *and* a search provider is configured,
   the shell calls `search_web`, imports each hit with `source_import_url`, and
   passes the resulting ids to step 1. The route is then planned against real
   material rather than against nothing. With no provider the toggle is
   inert and the form says so, instead of silently pretending to search.
1. **Create a route** — `course_create` inserts a `jobs` row and returns it.
   The frontend polls `job_get` (~220 ms) and renders real phases.
2. **Plan** — the job body runs `curriculum::plan()`: a deterministic planner
   that always produces a usable route. If an AI provider is configured, its
   output is validated against `ai/schemas.rs` and merged; on any failure the
   deterministic plan is kept. Six nodes by default, eight for `deep`, and the
   intro node is dropped for `advanced`.
3. **Open a node** — `course_node_prepare` assembles a `PreparedLesson`: the
   study steps, the concepts, the citation list, and (for review/quiz nodes) a
   generated question set. `lesson_store.prepare()` caches it per node.
4. **Answer** — `quiz_submit` grades the attempt. Objective kinds
   (`single`/`multi`/`fill`) are graded locally by `domain/quiz.rs`;
   `short`/`image` come back `graded: false` ("已提交 · 待评阅") and do not
   move the learner model. A free-text answer is not a dead end: the card
   offers 让 Orbit 评阅这段作答, which calls `learning_respond`, records a
   `learning_attempts` row, moves mastery, and puts the verdict in the chat.
   Evidence chunks sharpen that grade but are not required — a route planned
   before anything was imported can still be assessed.
5. **Diagnose** — two things can go wrong, and both are handled the same way:
   a wrong objective answer writes a `learner_states` row, and a tutor turn
   that names a misconception writes an `adaptation_events` row of kind
   `misconception_detected`. Either one can splice a review node in right
   after the node it belongs to. `course_get` then returns the new roadmap,
   the home screen shows a notice strip explaining the change, and the chat
   says so in the turn that caused it. The shared `加练 · …` title means the
   two paths can never stack duplicates on the same node.
6. **Finish** — `course_node_complete` marks the node completed, unlocks the
   next one, and returns the updated course plus the adaptation.
7. **Review** — `review_at` on `course_nodes` plus mastery in
   `learner_states` drive the `dueLabel` copy on the roadmap.

---

## 5. Local AI core (BYOK)

The local AI core is deliberately provider-neutral. Everything below runs in
the Rust backend; the frontend only sees typed DTOs and `configured` /
`maskedHint` provider metadata.

### 5.1 Provider boundary

```
ai/traits.rs             ChatProvider, EmbeddingProvider, ChatRequest/Response,
                         ChatDelta, streaming + embedding validation
ai/openai_compatible.rs  real HTTP provider: /v1/chat/completions,
                         /v1/responses, /v1/embeddings, SSE streaming
ai/mock.rs               MockProvider — deterministic offline fallback
ai/registry.rs           ProviderRegistry, BYOK secrets, capability probing
ai/capabilities.rs       ProviderCapabilities + preferred protocol
ai/http.rs               retry/timeout policy
ai/stream.rs             AiStreamEvent lifecycle
ai/structured.rs         structured generation + one repair retry
ai/prompts.rs            versioned prompts with explicit untrusted-content
                         boundaries
ai/schemas.rs            CurriculumPlan / TutorTurn / FreeTextEvaluation /
                         QuizGeneration / SourceSummary
```

* Protocol support is `auto | responses | chat_completions`. `auto` probes
  both endpoints and prefers Responses only when it actually answers; Chat
  Completions remains first-class for third-party gateways.
* `/v1/chat/completions` and `/v1/embeddings` are preserved as stable
  OpenAI-compatible surfaces. `/v1/responses` is additive.
* Capability detection is a real probe with minimal inference calls, cached in
  `provider_capability_cache`. It records chat, streaming, structured output,
  embeddings and the observed embedding dimension.
* No embedding dimension is hardcoded. The dimension comes from the configured
  provider, is persisted in `embedding_index_metadata`, and a change forces a
  reindex instead of silently mixing vector spaces.
* `MockProvider` is backend-only. It never pretends to be a real model: every
  artefact it produces is tagged `generatedBy = "mock"` and the UI says so.

### 5.2 Keys and secrets

* **Keys never reach the frontend.** `provider_upsert` writes the key into the
  OS keyring via `secrets/keyring_store.rs`. `provider_list` returns only
  `hasApiKey: boolean` and a masked hint. There is no command that returns a
  key, and `AppError` redacts anything that looks like a secret before it is
  serialised, logged or emitted in a stream event.

### 5.3 Ingestion and retrieval

* Imported material is **untrusted reference content**. It is passed to the
  model only in a clearly delimited `UNTRUSTED REFERENCE CONTENT` user section;
  it never enters the system instruction. Prompts are tested for this boundary.
* Source parse → chunk → embed → index is one pipeline. The embedding cache is
  keyed by `SHA-256(normalised text + provider identity + model + dimension
  config)`, so identical chunks are not re-embedded.
* Retrieval is hybrid: FTS5 lexical search plus the persistent SQLite vector
  index, merged with reciprocal-rank-style scoring. Route, source and knowledge
  point filters are applied in SQL, so one route cannot leak another route's
  material.
* URL ingestion rejects `localhost`, loopback, private, link-local and
  cloud-metadata addresses, plus `file:`/`ftp:` schemes, and re-validates DNS
  and every redirect before following it.

### 5.4 Tutor and citations

* The tutor retrieves route-scoped evidence, labels it `[C1]`, `[C2]`, … and
  maps labels back to real chunk/source rows in Rust. The model never sees or
  emits database IDs.
* Grounding is explicit: `source_grounded`, `model_knowledge`, or
  `insufficient_source` when the learner asks about "my materials" and nothing
  relevant was retrieved.
* The streaming tutor uses a plain-text prompt (`TUTOR_STREAM_V1`) so tokens
  can be rendered incrementally. An optional final `DIAGNOSIS: {...}` line is
  stripped before display and fed into the same adaptation pipeline as the
  non-streaming path.

### 5.5 One adaptation policy

```
Diagnosis -> AdaptationPolicy -> AdaptationDecision -> AdaptationService
```

Quiz grading, free-text evaluation and tutor diagnosis all call the same
`AdaptationService`. A real route change is idempotent per knowledge point and
review reason, including across a database close/reopen.

### 5.6 Verruckt Cloud as one provider

Verruckt Cloud is not a second AI stack. It is one more implementation behind
the existing `ChatProvider` / `EmbeddingProvider` traits:

```
cloud/client.rs          CloudAccountClient: auth, refresh, entitlements,
                         devices, base URL, sync setting, broadcast events
cloud/models.rs          CloudAccountState / CloudEntitlements / device DTOs
cloud/commands.rs        the cloud_* Tauri command surface
ai/cloud.rs              CloudAiProvider -> /v1/chat/completions and
                         /v1/embeddings with the current access token
```

* `AiMode` is `offline | byok | cloud`. `byok` preserves the pre-integration
  behaviour; `cloud` falls back to BYOK/mock when the session, entitlement or
  quota is unavailable, so a Cloud failure never blocks learning.
* Cloud protocol is intentionally **Chat Completions only**. Logical models
  `verruckt-fast`, `verruckt-standard`, `verruckt-reasoning` and
  `verruckt-embedding` are mapped by the Cloud gateway to real upstream models.
* The Cloud provider needs a `providers` row because local embeddings reference
  `providers(id)`. `ProviderRegistry::ensure_cloud_provider()` upserts that
  virtual row on first use; it is never shown as a user-configured BYOK service.
* Access tokens are memory-only. The refresh token lives in the OS keyring and
  is never sent to the frontend. A single-flight refresh lock refreshes on 401
  and retries the original request exactly once; an auth failure clears the
  session and emits `cloud_signed_out`.

### 5.7 Local-first delta sync

Sync is authoritative-state only, with the Cloud as a relay, never the source
of truth for learning:

* SQLite triggers journal authoritative writes into `sync_outbox` in the same
  transaction as the write. Derived data (chunks, embeddings, FTS, caches) is
  never synced and is rebuilt locally.
* `sync_apply_guard` stops pulled writes from re-entering the outbox.
  `schema_version = 1` is carried in every payload.
* Local UUIDs are stored as BLOBs by sqlx; the sync journal normalises them to
  lowercase hex text and the adapters bind them back as UUIDs. Composite local
  ids (`course_node_dependency`, `node_knowledge_point`, `learner_state`) map to
  deterministic UUIDv5 ids.
* Bootstrap and pull apply in foreign-key-safe order. A server revision that
  differs from the local base revision records a row in `sync_conflicts` and
  leaves the local row untouched; an open conflict also blocks later pulls for
  that entity, so the user never gets a silent overwrite.
* Sync triggers are debounced on startup, foreground/manual, and periodically;
  manual controls live in Settings. Failure is non-fatal and surfaced as
  friendly copy.

### 5.8 Source files through object storage

Source blobs are too large for the delta journal:

* `sync/files.rs` requests a presigned upload, PUTs the bytes directly to
  MinIO/S3, records the Cloud file id on the revision, and later presigns a
  download on another device.
* Both directions verify SHA-256 before trusting the bytes. Local blobs remain
  content-addressed, so the same bytes are deduplicated.
* Pulling a source revision only schedules a local re-index; chunks and
  embeddings are rebuilt on the receiving device.

---

## 6. Frontend architecture notes

### The swipe engine is not React

`src/lib/swipeEngine.ts` is a framework-free, imperative module:

* `bindSwipeZone(zone, spec)` attaches pointer / touch / wheel listeners and
  returns a disposer.
* Drag frames are written straight to `transform` inside
  `requestAnimationFrame` — a pointer move never triggers a React render.
* `detailDrag()` is the prototype's weighted physics (lead distance, detent
  band, resistance slopes, travel cap). `swipeTiming()` is its release curve.
* The engine is a module singleton with explicit lifecycle
  (`isSwipeBusy`, `clicksAreSuppressed`, `finishActiveSwipe`,
  `resetSwipeEngine`). `App.tsx` binds it once and feeds it a stable `SwipeSpec`
  whose methods read a `latest` ref, so data changes never rebind listeners.

### Preview stages

A swipe needs to show the *destination* before the route/tab actually changes.
`createPreview()` in `App.tsx` mounts a detached
`.swipe-preview-stage` with its own `createRoot`, renders
`RoadmapScreen` (preview) or `LessonPane` into it, and returns a `SwipePreview`.
On commit, `preview.adopt(commit)` runs the real state change inside
`flushSync`, then unmounts the stage — so there is never a frame where the
destination is missing.

### Lesson card → fullscreen transition

`src/lib/lessonTransition.ts` reproduces the prototype exactly:

* `lessonCardClip(bounds, frame)` builds the `inset(...)` clip from the tapped
  node's bounds.
* Forward: the roadmap is cloned into `.lesson-underlay` (first child of
  `#root`), React renders the lesson into `#screen`, `#screen` gets
  `.screen-opening` (z-26), and the clip animates open while the lesson's own
  content staggers in.
* Reverse: the lesson is cloned into `.lesson-opening` appended *after*
  `#screen`, and every animation runs with `direction: "reverse"` and reversed
  delays.
* Every animation shares `document.timeline.currentTime`, so the stagger stays
  in sync with the clip.
* Without the Web Animations API (or under `prefers-reduced-motion`) the
  transition is skipped and the view switches instantly — the screens are
  already mounted either way.

### Styling

Tailwind v4 is the primary styling system, configured CSS-first in
`src/styles/index.css` (`@theme` tokens: `--color-canvas`, `--color-paper`,
`--color-ink`, `--color-muted`, `--color-line`, `--color-accent`, …).
Orbit's ported CSS lives in `src/styles/components.css` inside a `components`
cascade layer, so Tailwind utilities always win. See `docs/DESIGN_SYSTEM.md`.

### Stores

| Store | Owns |
| --- | --- |
| `uiStore` | view (`home·lesson·learning·create·generating`), tab, active node, direction, toasts |
| `routeStore` | routes, the active route, per-route details, adaptations, scroll memory |
| `jobStore` | the active background job and its phase copy |
| `lessonStore` | prepared lessons, keyed by node id |
| `learningStore` | the live session: answered set, messages, draft, result, adaptation |
| `sourceStore` | imported sources and their ingestion jobs |

`routeStore.load()` deliberately loads **every** route's detail up front: a
swipe preview must render a neighbouring route synchronously, mid-gesture.

---

## 7. Testing

`src/test/ipc-contract.test.ts` is worth calling out. TypeScript and Rust share
nothing but a string, so a renamed command would compile on both sides and only
fail at runtime inside the WebView. That file parses `api.ts`, `lib.rs` and the
fake backend and asserts three things: every command the frontend calls is
registered in `generate_handler!`, every registered command is either called or
on an explicit `BACKEND_ONLY` list, and every called command is implemented by
the fake — so a test can never pass by silently hitting an unimplemented
command. It also asserts both parsed sets are non-empty, so the check cannot
pass vacuously.

| Layer | Where | What it proves |
| --- | --- | --- |
| Rust unit | `src-tauri/src/**` (67 tests) | repo CRUD, migrations, planner, grading, mastery, chunking, hashing, sync journal/adapters |
| Rust integration | `src-tauri/tests/learning_loop.rs` (14), `provider_compat.rs` (8), `retrieval.rs` (4), `cloud_client.rs` (7) | the learning loop, provider protocol matrix, hybrid retrieval, and Cloud session/refresh/error/provider gating against a loopback mock |
| Rust Cloud E2E | `src-tauri/tests/cloud_live.rs` (1 ignored by default) | real desktop → Docker Compose Cloud → mock OpenAI upstream: account, Cloud AI, embeddings, delta sync A→B, conflict preservation, file upload/download, restart persistence |
| Contract | `src/test/ipc-contract.test.ts` (3 tests) | command names agree across `api.ts`, `lib.rs` and the fake backend |
| Frontend | `src/test/app.test.tsx` (14 tests) | route switching, lesson open/close, quiz submit (correct / wrong / ungraded), model review of a free-text answer, route creation, web collection, BYOK save (key never echoed), source library, tutor adaptation |
| Frontend unit | `src/lib/design.test.ts` (13 tests) | road geometry, drag physics, release timing, clip math, node faces |

`src/test/fakeBackend.ts` implements the IPC contract in memory (including job
polling and adaptation inserts) rather than stubbing individual calls, so the
frontend tests exercise the real DTO shapes.

---

## 8. Secondary surfaces

Both are reached from the management layer (the acrylic sheet behind the route
title) and both are real screens, not stubs:

* **资料库** (`screens/SourcesScreen.tsx`) — lists every source a route can
  cite, with re-index and delete.
* **设置** (`screens/SettingsScreen.tsx`) — AI source (Cloud / BYOK / offline),
  Verruckt Cloud account, entitlements, devices, Cloud Sync toggle and manual
  sync, plus BYOK add / edit / test / remove and the web search provider. The
  key field is write-only and never prefilled; the Cloud refresh token is never
  returned by any command.

## 9. Known gaps

These are real and unfinished:

* `learning_respond` records an attempt and moves mastery, but a failed
  free-text answer does not yet insert practice the way a failed objective
  answer does — only the tutor's diagnosis does that.
* The tutor adapts by *inserting practice*. It cannot yet shorten, reorder or
  re-word the remaining steps of the lesson it is in.
* Web collection imports the top hits as sources and grounds the route in them,
  but it does not de-duplicate near-identical pages or re-rank by quality.
* A provider's API key can be tested and stored, but nothing verifies the
  embedding model actually exists on the configured endpoint until a job runs.
* Bundling is configured (`bundle.targets: "all"`) but only `--no-bundle` builds
  have been run in this environment, because the MSI/NSIS toolchain is not
  available offline.
* The live Cloud E2E is opt-in (`cargo test --test cloud_live -- --ignored`);
  it needs Docker Compose and a fresh pro account. A real WebView smoke against
  Cloud is not automated here.
* Sync conflict resolution currently preserves the local row and records the
  server payload. The UI warns about the open conflict but does not yet offer a
  side-by-side merge/choose-server action.
* Cloud AI streaming uses the same incremental stream path as BYOK, but the
  live Cloud E2E exercises non-streaming chat and embeddings; the SSE path is
  covered by the Cloud workspace integration test and the frontend unit path.
