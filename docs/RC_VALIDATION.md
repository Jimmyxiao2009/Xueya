# v0.1.0-rc.1 Release Candidate Validation

This record only contains checks that were actually executed during the
`COMPETITION RELEASE VALIDATION & FREEZE` round. Anything not executed is
marked `SKIPPED` with the technical reason.

## Environment

- Repository: `H:\项目\CrossPlatform\学涯`
- Branch: `feature/cloud-integration`
- Validation commits: `c1fa166`, `73fc1ac`
- Pre-validation archive: `archive/rc1-before-validation` (`156039a`)
- Pre-validation tag: `v0.1.0-rc.1-pre-validation` (`156039a`)
- Desktop: Tauri 2, React 19 + TypeScript + Tailwind, Rust local core
- Cloud: Rust/Axum workspace, PostgreSQL, MinIO/S3, Docker Compose
- WebView automation: Windows UI Automation (no `tauri-driver` /
  `msedgedriver` / `chromedriver` in this environment)
- Rust test jobs: `CARGO_BUILD_JOBS=1` to avoid the Windows page-file/mmap
  failure seen with default parallel test builds

## Baseline

| ID | Check | Result | Evidence |
| --- | --- | --- | --- |
| B1 | `npm run typecheck` | PASS | clean exit |
| B2 | `npm run lint` | PASS | clean exit |
| B3 | `npm run test` | PASS | 3 files, 30 tests |
| B4 | `npm run build` | PASS | Vite production build |
| B5 | `cargo fmt --check` (desktop) | PASS | clean exit |
| B6 | `cargo clippy --all-targets -- -D warnings` | PASS | clean exit |
| B7 | `cargo test` (desktop) | PASS | 71 unit + 12 cloud-client + 1 demo + 14 learning + 8 provider + 1 golden-path + 4 retrieval |
| B8 | `cargo fmt --all -- --check` (cloud) | PASS | clean exit |
| B9 | `cargo clippy --workspace --all-targets -- -D warnings` | PASS | clean exit |
| B10 | `cargo test --workspace` (cloud) | PASS | 3 + 5 + 2 + 13 + 3 tests |

## Installer / Migration / Profile

| ID | Check | Result | Evidence |
| --- | --- | --- | --- |
| I1 | Fresh NSIS install | PASS | `Orbit_0.1.0-rc.1_x64-setup.exe` installs to `%LOCALAPPDATA%\Orbit` |
| I2 | Fresh launch after install | PASS | Release app launches; previous first-launch crash fixed by `c1fa166` |
| I3 | Restart installed app | PASS | Reopens profile and local data |
| I4 | Uninstall `/S` | PASS | Exit 0; install dir and Start Menu shortcut removed; app data preserved |
| I5 | Reinstall after uninstall | PASS | Reinstalls and launches |
| I6 | Existing-profile migration fixture | PASS | Migrations 1..5 applied; fixture counts preserved (`courses=2 nodes=12 sources=1 adaptations=5`); `cloud_account_snapshot` created |
| I7 | Uninstall registry entry | PASS | `HKCU\...\Uninstall\Orbit`, DisplayName `Orbit`, Publisher `verruckt`, InstallLocation `%LOCALAPPDATA%\Orbit` |
| I8 | Binary metadata | PASS | ProductName `Orbit`, FileVersion/ProductVersion `0.1.0-rc.1`; installed exe CompanyName `verruckt` |
| I9 | NSIS SHA-256 | PASS | `b7f332287ec35c8019ae9542da1805b50a48caf4e08cfd287e9bece2d16957ee` |
| I10 | Installed exe SHA-256 | PASS | `b488a8b1653a1f1ffffcbfdb040e5c00fca876e41dc3a92c63d3e3e1a324d7f5` |
| I11 | MSI bundle | SKIPPED | Tauri rejects the non-numeric pre-release identifier `rc.1`; changing it would break the frozen version. NSIS is the supported Windows artifact. |

## Golden Path / WebView

| ID | Check | Result | Evidence |
| --- | --- | --- | --- |
| G1 | Rust Golden Path 10x | PASS | `rc_golden_path.rs`; 10 independent TempDir profiles, real start/turn/quiz/drop/reopen; review node persisted each iteration |
| G2 | Real WebView Golden Path 3x | PASS | UIA automation on release app; route selected, lesson started, grounded tutor citation, wrong option B, adaptation feedback, review node count exactly 1 |
| G3 | Competition Demo Reset 5x | PASS | Dev build with `VERRUCKT_COMPETITION_DEMO=1`; two confirmations per reset; `demoRoutes=1 courses=3 sources=3 nodes=18 demoNodes=6` each time; normal seeded routes survived |
| G4 | Demo isolation | PASS | 5x resets preserved non-demo seeded routes; existing demo test asserts one demo route and real source grounding |

## Offline

| ID | Check | Result | Evidence |
| --- | --- | --- | --- |
| O1 | Local core offline | PASS | Docker Compose stopped; `competition_demo`, `learning_loop` (14 tests), `rc_golden_path` (10/10) all pass |
| O2 | Real WebView offline Golden Path | PASS | Docker Compose stopped; release WebView Golden Path 3/3 pass with local fixture citation and adaptation |
| O3 | Offline provider honesty | PASS | UI labels offline/mock replies; no claim of Cloud AI while offline |
| O4 | Cloud restore after offline | PASS | Live desktop test: `DegradedOffline -> Authenticated` without re-login after cloud restart |

## Recovery / Fault Matrix

Live desktop fault matrix was run against Docker Compose with a temporary
ignored integration test (not committed). The service-level matrix was run
with PowerShell + Docker + HTTP.

| ID | Scenario | Result | Evidence |
| --- | --- | --- | --- |
| F1 | Cloud up / AI up / storage up | PASS | Live `cloud_live` account, AI chat, SSE, sync, file upload/download, restart persistence |
| F2 | Cloud down | PASS | Desktop `force_refresh` -> `DegradedOffline`; last email/plan retained; local route creation still succeeds |
| F3 | Cloud restored | PASS | Desktop `account_state` -> `Authenticated` without re-login; service health restored |
| F4 | AI upstream down | PASS | Account `/me` still works; chat fails with non-auth error; no `SignedOut` |
| F5 | AI upstream restored | PASS | Chat recovers without re-login/app restart |
| F6 | Object storage down | PASS | Local source remains; file sync fails/waits; no local source deletion |
| F7 | Object storage restored | PASS | File sync uploads again |
| F8 | Refresh revoked/rejected | PASS | `SignedOut`, keyring refresh token deleted, local learning still works |
| F9 | Cloud service restart persistence | PASS | Account identity, entitlement, sync/file metadata survive container restart |
| F10 | Close during index crash injection | SKIPPED | No deterministic process-kill hook in this environment. Code review found `ingest_bytes` marks the source `Ready` before `embed_chunks` finishes, so an interrupted first index can leave a ready source with partial vectors. Retrieval still degrades to lexical, and `重新索引` is available. Recorded as post-competition hardening. |

## RAG / Citation / Prompt Safety

| ID | Check | Result | Evidence |
| --- | --- | --- | --- |
| R1 | Hybrid retrieval route isolation | PASS | `retrieval.rs::hybrid_retrieval_finds_projective_infinity_and_keeps_routes_isolated` |
| R2 | Citation maps to retrieved chunk | PASS | `retrieval.rs::streaming_tutor_citations_map_back_to_the_retrieved_chunk` |
| R3 | Real WebView citation | PASS | Golden Path UIA found `查看依据：射影平面与无穷远点`; expanded excerpt matched the local fixture |
| R4 | Prompt injection boundary | PASS | `ai::prompts` unit tests keep untrusted text out of the system prompt and ignore `IGNORE ALL PREVIOUS INSTRUCTIONS` in source material |
| R5 | 1000-chunk hybrid search | PASS | Temporary smoke test: 1000 chunks, mock embeddings, search 18 ms, 5 hits |
| R6 | Long streaming response | PASS | Frontend coalesces deltas; `cloud_client` cancellation/terminal tests pass; live SSE lifecycle pass |

## Adaptation

| ID | Check | Result | Evidence |
| --- | --- | --- | --- |
| A1 | Wrong answer inserts review | PASS | `learning_loop` adaptation tests + WebView Golden Path |
| A2 | Adaptation persists across DB reopen | PASS | `an_adaptation_survives_a_database_reopen_and_stays_idempotent` |
| A3 | Duplicate prevention | PASS | Same diagnosis after reopen resolves to the same review node; Golden Path review count exactly 1 each iteration |
| A4 | Review node title | PASS | WebView observed `加练 · 无穷远点` |

## Responsive / VisualViewport

| ID | Check | Result | Evidence |
| --- | --- | --- | --- |
| V1 | Widths 360 / 390 / 430 / 480 | PASS | UIA overflow checker: Home, RoadmapLayer, Sources, Settings, CreateRoute, Lesson, Learning all had zero horizontal overflow beyond a 4 px UIA rounding tolerance |
| V2 | Reduced viewport height 600 | PASS | Lesson `开始学习` and Learning `chat-input` / `发送消息` stayed inside the reduced-height viewport |
| V3 | Actual software keyboard | SKIPPED | WebView2 keyboard overlay cannot be simulated without a WebDriver/VisualViewport driver; reduced-height smoke is the documented substitute |
| V4 | Pixel-perfect visual comparison | SKIPPED | No screenshot diff baseline/comparator in this environment; functional UIA geometry checks were used instead |

## Accessibility

| ID | Check | Result | Evidence |
| --- | --- | --- | --- |
| Y1 | Keyboard route switch / route select / node / start | PASS | Focus + Enter opened the lesson; all targets `IsKeyboardFocusable=True` |
| Y2 | Composer keyboard | PASS | `chat-input` has `AutomationId=chat-input` and accessible name `发送消息`; send button focusable and Enter submitted |
| Y3 | Citation keyboard expansion | PASS | Citation button `查看依据：射影平面与无穷远点` focusable; Enter changed `ExpandCollapseState` to `Expanded` |
| Y4 | Accessible names / roles | PASS | Core buttons, tabs, citation, source actions, quiz feedback have readable names; quiz feedback uses `role="status"` |
| Y5 | Cloud / sync dynamic status live region | SKIPPED | Status text is readable but has no explicit `aria-live`/`role="status"`; recorded as LOW post-competition backlog, not a competition blocker |
| Y6 | Reduced motion | PASS (source/tests) | `useReducedMotion`, CSS media queries, and component branches exist; no OS-level runtime toggle was available for a visual pass |

## Security / Secret Audit

| ID | Check | Result | Evidence |
| --- | --- | --- | --- |
| S1 | Error redaction | PASS | `error::tests::serialization_has_stable_shape_and_redacts_secrets` |
| S2 | No direct secret logging | PASS | Static search found no `tracing`/`println` of API key, refresh token, access token, or Authorization header |
| S3 | Local DB secret audit | PASS | Production DB `cloud_account_snapshot` empty; `app_settings` contains only `ai_provider_mode=offline`; no key/token rows |
| S4 | Sync payload audit | PASS | Delta sync carries local domain entities; file sync carries source files and SHA-256; embeddings/API keys/refresh tokens are never synced |
| S5 | Keyring first-launch handling | PASS | Fixed in `c1fa166`; Windows `No matching credential found` is treated as absent |

## Known Non-Blocking Gaps

- MSI cannot be built while the version is `0.1.0-rc.1`; NSIS is the release
  artifact.
- NSIS wrapper metadata leaves `CompanyName` blank; the installed exe has
  `CompanyName=verruckt`.
- Close-during-first-index can leave a source `Ready` with partial vectors;
  lexical retrieval and manual re-index still work. Post-competition hardening.
- Cloud/sync dynamic status text lacks an explicit live region.
- Pixel-perfect visual diff and actual software-keyboard overlay were not
  available in this environment; functional geometry and reduced-height checks
  were used instead.

## Verdict

The four competition-critical areas pass: installer/launch/restart/uninstall/
reinstall, Golden Path, offline fallback, and local data persistence. The
non-blocking gaps above are recorded for post-competition work.

`READY FOR COMPETITION`
