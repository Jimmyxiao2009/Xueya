# Product convergence implementation plan

## Baseline (2026-09-20)
- Existing repository, branch `main`, extensive pre-existing tracked and untracked work.
- Preserved baseline status and tracked diff in `audit-results/product-convergence/`; no reset, checkout, cleanup or commit of user work.
- Native Rust Local Core and separate Rust Cloud already exist. `web-server` is an opt-in feature but frontend onboarding currently gates native startup on Cloud.
- Confirmed P0: scoped course retrieval falls through globally on empty/error; normal no-source courses/lessons bypass AI; Cloud presence misreports mock state; template lesson inherits AI roadmap origin.
- Baseline frontend tests/build and Rust library tests captured before changes.

## Ordered execution / acceptance checklist
- [ ] Phase 1: source isolation; real-provider AI regardless of evidence; truthful fallback/cached provenance; require-AI enforcement; native local start. Regression tests before visual reconstruction.
- [ ] Phase 2: document Local Core / optional Account+Sync+Gateway / managed browser runtime boundaries; remove native account gate.
- [ ] Phase 3: persisted motif × scheme with OS resolution and semantic tokens.
- [ ] Phase 4: conversational OOBE, navigation anchors/states, reading surfaces, settings, desktop adaptation; consolidate style ownership.
- [ ] Phase 5: execute auth, refresh, provider, streaming, sync/outbox/conflict tests; distinguish simulated upstream coverage from live Cloud verification.
- [ ] Phase 6: remove overridden CSS and misleading product documentation; declare Node/Rust and CI toolchains.
- [ ] Phase 7: full available checks, visual QA where feasible, six deliverable reports with explicit UNVERIFIED items.

## Verification policy
No production account credentials are assumed. Tests against loopback servers prove behavior, not availability of a real AI service. Report live authentication, official AI and native installation separately. Preserve existing demo account isolation: a browser managed server is not an offline local device.

## Convergence pass status (competition build)

- [x] Conversation-first learning: agent opening turn, conversation primary,
  quiz optional via `阶段检查`.
- [x] First-class session/turn domain view: `LearningTurn`, `AgentAction`,
  `LearnerResponse`, `ConceptState`, `MasteryEvidence`.
- [x] Explicit AI policy: Offline / BYOK / Cloud; no silent Mock fallback;
  deterministic roadmap fallback only in explicit Offline mode.
- [x] Account optional: native local start, local learning and local restart
  do not require login.
- [x] Cloud boundary: Account / Device / Entitlement / AI Gateway / Sync /
  Storage / Quota; no Cloud import of local learning services.
- [x] Windows build/test evidence captured; Browser Demo compile captured.
- [ ] Android device smoke test and live Cloud/PostgreSQL E2E remain for the
  release owner; see `audit/competition-convergence/COMPETITION_CONVERGENCE_REPORT.md`.