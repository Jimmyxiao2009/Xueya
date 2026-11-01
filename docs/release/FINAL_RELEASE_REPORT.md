# FINAL RELEASE REPORT

## RELEASE STATUS

**READY WITH KNOWN LIMITATIONS**

本地可完成的 production/demo build、脚本、健康检查、Smoke Test、Windows installer 与回归均已完成。剩余只依赖外部生产 Cloud URL、真实 Demo 账号、真实 upstream model key 和 Windows 签名环境。

## Build

- **Windows installer:** `release-artifacts\学涯_0.1.0-rc.1_x64-setup.exe`
- **Generated installer:** `src-tauri\target\release\bundle\nsis\学涯_0.1.0-rc.1_x64-setup.exe`
- **Executable:** `src-tauri\target\release\verruckt.exe`
- **Cloud endpoint embedded:** `http://127.0.0.1:8080` (local validation build only; production build must use a real HTTPS URL)
- **Signed:** NO / UNSIGNED WINDOWS INSTALLER
- **SHA256:** `371F81742E8165B104164A3A32BE9F3129684269D33A170C05B42F28FE20EBFE`

## Release Package

- Source archive: `release-package/学涯-Orbit-Verruckt-source-0.1.0-rc.1.zip`
- Source SHA256: see `release-package/SHA256SUMS.txt`
- Binaries: `release-package/binaries/学涯_0.1.0-rc.1_x64-setup.exe`
- Docs: `release-package/docs/`
- Hashes: `release-package/SHA256SUMS.txt`
- Freeze manifest: `audit/final-release/FREEZE_MANIFEST.md`

## Production Cloud

| Item | Status |
|---|---|
| Reachability | TEST VERIFIED (local Docker Compose) |
| Account | TEST VERIFIED (local Docker Compose) |
| AI Gateway | TEST VERIFIED (local Docker Compose + mock upstream) |
| Real upstream | NOT VERIFIED |
| Sync | TEST VERIFIED (local Docker Compose A -> Cloud -> B) |
| Quota | TEST VERIFIED (local Docker Compose) |

## Socratic Demo

| Item | Status |
|---|---|
| Opening | LIVE VERIFIED (local Docker Compose) |
| Wrong-answer adaptation | LIVE VERIFIED (local Docker Compose) |
| Direct explain | LIVE VERIFIED (local Docker Compose) |
| Plan progress | TEST VERIFIED (frontend + Rust) |
| Quiz-free | TEST VERIFIED |
| Resume | LIVE VERIFIED (10-turn session restart/resume) |

## Scripts

| Script | Status |
|---|---|
| `build-with-cloud.ps1` | VERIFIED (built NSIS installer; invalid URL / HTTP production URL fail) |
| `build-competition.ps1` | VERIFIED (missing URL fails) |
| `pre-demo-check.ps1` | VERIFIED (PASS local; wrong URL / wrong login / AI unavailable fail) |
| `run-demo-smoke.ps1` | LocalCloud VERIFIED; Remote NOT VERIFIED (requires external Cloud + real upstream) |
| `build-with-cloud.sh` | NOT VERIFIED (no bash in this Windows workspace) |

## Regression

- `npm run typecheck` — PASS
- `npm run lint` — PASS
- `npm test` — PASS: 17 files / 89 tests
- `npm run build` — PASS
- `cargo test --lib` — PASS: 82 tests
- `cargo test --test learning_loop --test rc_golden_path` — PASS: 18 tests
- `cargo test --test provider_compat --test cloud_first_policy` — PASS: 21 tests
- `cargo test --test cloud_client -- --test-threads=1` — PASS: 16 tests
- `pwsh -File scripts/e2e-cloud.ps1` — PASS: 2 live Cloud tests
- `cargo check` cloud / vector / orbit-server — PASS
- `cargo fmt --check` — historical formatting drift only (`provider_compat.rs`, `retrieval.rs`)
- `cargo clippy --lib --no-deps` — historical `explicit_auto_deref` warnings only

## Remaining limitations

1. Production/staging Cloud URL + real Demo account not available in this workspace.
2. Real upstream model vendor not available; live Cloud E2E used mock OpenAI-compatible upstream.
3. Windows installer is unsigned.
4. `build-with-cloud.sh` not executed on this Windows-only workspace.
5. Android remains STATIC REVIEW ONLY.

## External input required

```text
production/staging Verruckt Cloud HTTPS URL
valid Demo account with official_ai + cloud_sync + quota
real upstream model key (if not already configured server-side)
Windows signing certificate (optional)
```

**WAITING ONLY FOR EXTERNAL CREDENTIAL/ENDPOINT**
