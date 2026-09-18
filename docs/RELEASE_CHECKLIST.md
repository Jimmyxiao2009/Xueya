# v0.1.0-rc.1 Release Checklist

只记录已经真实执行的项目。`PASS` 代表本轮有命令/自动化/人工可复现证据；
`SKIPPED` 代表环境缺少必要条件，且已说明技术原因。

## 冻结与保护

- [x] PASS 验证开始时工作树干净（只有后续新增的验证文档/校验和）
- [x] PASS 创建 `archive/rc1-before-validation`（`156039a`）
- [x] PASS 创建 tag `v0.1.0-rc.1-pre-validation`（`156039a`）
- [x] PASS 未执行 `git reset --hard`，未重写历史

## Desktop 基线

- [x] PASS `npm run typecheck`
- [x] PASS `npm run lint`
- [x] PASS `npm run test`（3 files / 30 tests）
- [x] PASS `npm run build`
- [x] PASS `cargo fmt --check`
- [x] PASS `cargo clippy --all-targets -- -D warnings`
- [x] PASS `cargo test`（71 unit + 12 cloud_client + 1 demo + 14 learning + 8 provider + 1 golden_path + 4 retrieval）

## Cloud 基线

- [x] PASS `cargo fmt --all -- --check`
- [x] PASS `cargo clippy --workspace --all-targets -- -D warnings`
- [x] PASS `cargo test --workspace`（3 + 5 + 2 + 13 + 3 tests）

## 安装、迁移、卸载

- [x] PASS 全新 NSIS 安装、启动、重启、卸载、重装
- [x] PASS 现有 profile migration fixture：migration 1..5 全部应用，业务计数保留，`cloud_account_snapshot` 创建
- [x] PASS 卸载 `/S` 退出 0，安装目录与开始菜单快捷方式移除，用户数据保留
- [x] PASS 注册表卸载项：`DisplayName=Orbit`、`Publisher=verruckt`、`InstallLocation=%LOCALAPPDATA%\Orbit`
- [x] PASS exe metadata：`ProductName=Orbit`、`FileVersion/ProductVersion=0.1.0-rc.1`、`CompanyName=verruckt`
- [x] PASS NSIS SHA-256 与 installed exe SHA-256 已写入 `RELEASE_CHECKSUMS.txt`
- [ ] SKIPPED MSI：Tauri 拒绝非纯数字预发布标识 `rc.1`；NSIS 为 rc.1 正式 Windows 产物

## Golden Path / 竞赛演示

- [x] PASS Rust Golden Path 10/10（真实 start/turn/quiz/drop/reopen，review node 持久化）
- [x] PASS WebView Golden Path 3/3（UIA；真实 citation、错题 adaptation、review node=1）
- [x] PASS 竞赛演示 reset 5x（dev/competition build；两次确认；普通路线保留）
- [x] PASS 演示路线第一站 `Completed`、第二站 `Available`
- [x] PASS 演示隔离：reset 只删除 `competitionDemo` 标记路线

## 离线

- [x] PASS Docker Compose 停止时 local core tests 全通过（demo + learning_loop + golden_path）
- [x] PASS Docker Compose 停止时 WebView Golden Path 3/3
- [x] PASS 离线 mock 回答与本地 citation 正常
- [x] PASS Cloud 恢复后自动 `DegradedOffline -> Authenticated`，无需重新登录

## 故障矩阵

- [x] PASS Cloud Up / AI Up / Storage Up：live account + AI + SSE + sync + file + restart persistence
- [x] PASS Cloud Down：`DegradedOffline`，账号快照保留，本地学习继续
- [x] PASS Cloud Restored：自动恢复 `Authenticated`，无需重新登录
- [x] PASS AI Down：Account PASS / Sync PASS / AI graceful fail（非 401）
- [x] PASS AI Restored：chat 恢复，无需重启
- [x] PASS Storage Down：本地 source 保留，file sync 等待/失败可重试
- [x] PASS Storage Restored：file sync 上传恢复
- [x] PASS Refresh Revoked：`SignedOut`，keyring refresh token 删除，本地学习不受影响
- [x] PASS Cloud container restart：账号/权益/sync/file metadata 持久化
- [ ] SKIPPED Close during first index：无确定性进程 kill hook；代码审查发现 `ingest_bytes` 在 `embed_chunks` 完成前就把 source 标为 `Ready`，中断首轮索引可能留下 partial vectors。检索会退回 lexical，且 UI 提供 `重新索引`。列入 post-competition hardening。

## RAG / Citation / Prompt Safety

- [x] PASS route-scoped hybrid retrieval
- [x] PASS streaming tutor citation 映射回真实 chunk
- [x] PASS WebView 展开 citation excerpt 与本地 fixture 一致
- [x] PASS prompt injection 边界：source 中的 `IGNORE ALL PREVIOUS INSTRUCTIONS` 不进入 system policy
- [x] PASS 1000-chunk hybrid search smoke：search 18 ms，5 hits
- [x] PASS long streaming / cancel：前端 delta coalescing；SSE cancel、malformed、upstream error 测试通过

## Adaptation

- [x] PASS 错题触发路线加练
- [x] PASS adaptation DB reopen 后仍存在
- [x] PASS 重复 diagnosis 幂等，review node 数量保持 1
- [x] PASS review node 标题 `加练 · 无穷远点`

## Responsive / VisualViewport

- [x] PASS 360 / 390 / 430 / 480 宽度：Home、RoadmapLayer、Sources、Settings、CreateRoute、Lesson、Learning 无水平溢出（UIA 几何检查，4 px rounding tolerance）
- [x] PASS 600 px reduced height：Lesson 开始学习、Learning chat-input/send 仍在 viewport 内
- [ ] SKIPPED 实际软键盘 VisualViewport overlay：环境无 WebDriver/VisualViewport 驱动
- [ ] SKIPPED 像素级视觉 diff：无 screenshot baseline/comparator

## Accessibility

- [x] PASS 键盘激活 route switch / route select / node / start
- [x] PASS composer `chat-input`、`发送消息` focusable，Enter 可提交
- [x] PASS citation `查看依据：…` focusable，Enter 后 `ExpandCollapseState=Expanded`
- [x] PASS 核心按钮/tab/citation/quiz feedback 有可读 accessible name；quiz feedback 使用 `role="status"`
- [ ] SKIPPED Cloud/Sync 动态状态没有显式 `aria-live`/`role="status"`；记录为 LOW backlog，不阻塞竞赛
- [x] PASS Reduced motion：hook + CSS media query + component branches；无 OS 级运行时切换，因此只做 source/test 级验证

## Security / Secret Audit

- [x] PASS 错误序列化脱敏测试
- [x] PASS 静态搜索未发现 API key / refresh token / access token / Authorization 日志
- [x] PASS 生产 DB 无 secret snapshot；`app_settings` 只有非敏感模式
- [x] PASS sync payload 不包含 API key、token、keyring secret、embedding vector
- [x] PASS keyring 首次启动缺失 credential 修复（`c1fa166`）

## 最终产物

- [x] PASS NSIS：`src-tauri/target/release/bundle/nsis/Orbit_0.1.0-rc.1_x64-setup.exe`
- [x] PASS installed exe：`%LOCALAPPDATA%\Orbit\verruckt.exe`
- [x] PASS `RELEASE_CHECKSUMS.txt`
- [x] PASS `docs/RC_VALIDATION.md`
- [x] PASS `docs/COMPETITION_DEMO.md`（含 3–5 分钟现场路径与 emergency fallback）
- [x] PASS 最终 freeze tag `v0.1.0-rc.1-competition`

## 非目标

本轮不新增支付、OAuth、CRDT、多租户、数据库架构、页面、RAG 算法或 UI 大改。

