# Verruckt architecture boundaries

Verruckt is a Local-First AI Learning System.

| Boundary | Responsibility | Implementation |
| --- | --- | --- |
| Local Core | SQLite learning database, course runtime, roadmap/state/progress/review, tutor context, source ingestion, retrieval, vector index, history, outbox | `src-tauri/src/services`, `domain`, `curriculum`, `knowledge`, `db`, `sync` |
| Verruckt Cloud | Account / identity / device / entitlement; incremental sync and file backup; authenticated AI gateway, quota, usage and streaming | `cloud/` |
| AI Provider Layer | Select Cloud, BYOK or explicit offline implementation; model protocol, credentials and generated-content provenance | `src-tauri/src/ai` |
| Managed Web Demo Runtime | Browser demonstration compatibility architecture; server-hosted AppState because browser sandbox cannot reproduce native core | opt-in `web-server` feature, `orbit-server`, `/api/invoke` |

Desktop/native = Local Core + optional Verruckt Cloud. Model calls can go through the Cloud; context assembly, source boundaries, business decisions and learning state remain local. Cloud is not a learning engine or the sole learning database.

Native startup and learning do not require an account. Cloud recovery runs independently, offline writes stay in SQLite/outbox, and sync conflicts preserve local and remote information. Browser demo authentication protects server-hosted account workspaces and does not define native access policy. Browser demo requires a reachable server and is not presented as native offline storage.

An explicit source selection is a closed retrieval boundary, including empty results and legacy contaminated node metadata. Topic-only creation does not implicitly consent to searching the entire local library. General-knowledge generation carries a separate provenance from source-grounded generation. Offline/mock output never receives a real-AI label.

Access tokens remain in Rust memory; native refresh tokens use the existing SecretStore. The frontend never receives credentials. Embeddings/chunks remain derived local data; source blobs retain hash verification. Learning progress must not be silently overwritten on sync conflict.

Theme = Visual Motif × Color Scheme. Lighthouse/Buoy are independent of Light/Dark/System. Quiet paper surfaces belong to reading content in either scheme.

## Competition convergence addendum

- The classroom is the conversation, not the quiz. A learning session starts
  with an agent-authored teaching turn; quiz remains an optional checkpoint.
- `LearningTurn`, `AgentAction`, `LearnerResponse`, `ConceptState` and
  `MasteryEvidence` are first-class domain concepts. Persistence currently maps
  them onto `learning_sessions`, `learning_messages`, `learning_attempts` and
  `learning_mastery`; a separate turn table is a future migration, not a runtime
  requirement.
- AI mode is explicit: `Offline` may use deterministic `MockProvider` and must
  be labelled; `BYOK` and `Cloud` must fail loudly instead of silently falling
  back to Mock. `ORBIT_REQUIRE_AI=1` forces a real provider or an error.
- Deterministic roadmap fallback is only allowed in explicit `Offline` mode.
- `orbit-server` remains Browser Demo / Dev Preview transport over the real
  local core `AppState`; it is not the native offline runtime.
- Windows and Android share the IPC/DTO surface. Android defaults to the SQLite
  vector backend; the `VectorIndex` interface stays consistent.