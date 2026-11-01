# FREEZE MANIFEST

- Version: `0.1.0-rc.1`
- Branch: `main`
- HEAD: `9dcc5ac30be1ffd5901faea3bbc443c2c792a5a4`
- Freeze date: `2026-09-21` (workspace date)
- Status: **PACKAGED WITH KNOWN LIMITATIONS**

## Artifacts

- Installer: `release-artifacts/学涯_0.1.0-rc.1_x64-setup.exe`
- Installer SHA256: `371F81742E8165B104164A3A32BE9F3129684269D33A170C05B42F28FE20EBFE`
- Source archive: `release-package/学涯-Orbit-Verruckt-source-0.1.0-rc.1.zip`
- Source archive SHA256: see `release-package/SHA256SUMS.txt`
- Package hashes: `release-package/SHA256SUMS.txt`
- Release report: `audit/final-release/FINAL_RELEASE_REPORT.md`
- Cleanup manifest: `audit/final-release/CLEANUP_MANIFEST.md`

## Tests

- `npm run typecheck` PASS
- `npm run lint` PASS
- `npm test` PASS: 17 files / 89 tests
- `npm run build` PASS
- `cargo test --lib` PASS: 82 tests
- `cargo test --test learning_loop --test rc_golden_path` PASS: 18 tests
- `cargo test --test provider_compat --test cloud_first_policy` PASS: 21 tests
- `cargo test --test cloud_client -- --test-threads=1` PASS: 16 tests
- `scripts/e2e-cloud.ps1` PASS: 2 live Cloud tests
- `cargo check` cloud / vector / orbit-server PASS

## Known limitations

- No public hosted Verruckt Cloud instance included.
- Real production upstream model not included/tested.
- Windows installer unsigned.
- Android static review only.
- `build-with-cloud.sh` not executed on this Windows-only workspace.
