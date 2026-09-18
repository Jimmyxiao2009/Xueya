# v0.1.0-rc.1 Release Checklist

本清单只记录已经真实执行并通过的项目；未执行的项目保持未勾选。

## Desktop 基线

- [x] `npm run typecheck`
- [x] `npm run lint`
- [x] `npm run test`
- [x] `npm run build`
- [x] `cargo fmt --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo test`

## Cloud 基线

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace`

## 已覆盖的恢复/故障路径

- [x] 网络错误/5xx 恢复会话失败 → `DegradedOffline`，保留 refresh token 与账号快照
- [x] 401/refresh 失效 → `SignedOut`，删除 keyring refresh token
- [x] 8 个并发请求共享 1 次 refresh
- [x] Cloud SSE 正常序列：`Started → Delta* → Completed`
- [x] malformed SSE → 终止为 `Failed`，不会假 `Completed`
- [x] upstream 5xx → 返回可恢复错误，不永久 running
- [x] 客户端 drop stream → 不 panic、不阻塞
- [x] 竞赛演示 reset 重建真实本地资料路线
- [x] 竞赛演示第一站完成、第二站当前

## 构建与 Bundle

- [x] `cargo build --release` 成功，产物 `src-tauri/target/release/verruckt.exe`
- [x] Windows NSIS 安装包成功：
      `src-tauri/target/release/bundle/nsis/Orbit_0.1.0-rc.1_x64-setup.exe`
- [x] productName `Orbit`、窗口标题 `Orbit · 学习的下一站`、identifier
      `app.verruckt.desktop`、版本 `0.1.0-rc.1`
- [ ] MSI：Tauri 拒绝 `0.1.0-rc.1`，原因是 MSI 的预发布标识必须为纯数字
- [ ] 安装、启动、建库、重启、卸载流程未在本机执行

## 仍需在目标机器/完整环境验证

- [ ] WebView 自动化 smoke（tauri-driver / WebDriver 可用时）
- [ ] 10 次 Golden Path 业务循环
- [ ] Docker Compose 全链路 live E2E（`cargo test --test cloud_live -- --ignored`）
- [ ] MinIO 宕机、Postgres/Cloud 重启、网络恢复后的完整故障矩阵
- [ ] 360/390/430/480 宽度与 a11y 回归

## 已知非目标

- 本轮不新增支付、OAuth、CRDT、多租户、数据库迁移或 UI 组件库。
- 本地优先、Cloud 可选、BYOK 一等公民、离线学习始终可用。
