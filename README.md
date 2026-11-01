# 学涯 / Orbit

**Codename:** Verruckt

学涯（Orbit）是一个以苏格拉底式连续对话为核心的 AI 学习应用。

Roadmap 提供学习方向；Learning Agent 通过解释、提问、追问、提示、例子与反例陪用户完成知识理解。学习 Runtime 保持本地优先；Verruckt Cloud 提供账号、设备、权益、AI Gateway、同步、存储与配额等增强服务。

```text
Local-first learning runtime
+
Cloud-first service experience
+
Conversation-first Socratic Tutor
```

## 架构

- **Local Core**：Course、Roadmap、LearningSession、Conversation、Concept/Mastery、Retrieval、Vector、SQLite 与学习闭环。
- **Verruckt Cloud**：Account、Device、Entitlement、AI Gateway、Sync、Storage / Backup、Quota。
- Cloud 不承载学习权威状态；本地 Core 始终是学习 Runtime。

## 快速启动

```bash
npm install
npm run tauri dev
```

本地开发默认可以使用 localhost Cloud；正式构建必须提供合法 HTTPS Cloud endpoint。

## 测试

```bash
npm run typecheck
npm run lint
npm test
npm run build
```

Rust 重点回归：

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo test --manifest-path src-tauri/Cargo.toml --test learning_loop --test rc_golden_path
cargo test --manifest-path src-tauri/Cargo.toml --test provider_compat --test cloud_first_policy
cargo test --manifest-path src-tauri/Cargo.toml --test cloud_client -- --test-threads=1
```

本地 Cloud E2E：

```powershell
.\scripts\e2e-cloud.ps1
```

## Windows 构建

```powershell
.\scripts\build-with-cloud.ps1 `
  -CloudUrl "https://your-verruckt-cloud.example"
```

该脚本会：

1. 验证 URL。
2. 将 `VERRUCKT_DEFAULT_CLOUD_URL` 编译进客户端默认 endpoint。
3. 执行前端校验与 Tauri build。
4. 输出 NSIS installer 与 executable 路径。

产物：

```text
src-tauri\target\release\bundle\nsis\学涯_0.1.0-rc.1_x64-setup.exe
```

比赛 wrapper：

```powershell
.\scripts\build-competition.ps1 -CloudUrl "https://your-verruckt-cloud.example"
```

## Cloud endpoint 可替换

客户端 endpoint 是 **build-time deployment configuration**：

```text
runtime override: VERRUCKT_CLOUD_URL
↓
persisted Settings value
↓
compile-time default: VERRUCKT_DEFAULT_CLOUD_URL
```

部署者可以为不同 Cloud Endpoint 重新构建客户端。普通产品 UI 不暴露任意服务器热切换，也不自动迁移不同 remote identity 的数据。

## Verruckt Cloud 部署

Cloud 服务端实现与容器化配置随源码提供：

```text
cloud/
deploy/
docs/CLOUD_DEPLOYMENT.md
```

由于项目开发阶段未长期部署公共测试服务器，评审方可自行部署 Cloud，并通过构建参数指定 Cloud Endpoint 重新生成客户端。

本地 Docker Compose E2E 已覆盖：

- Account / Device / Entitlement
- AI Gateway
- Sync A → Cloud → B
- Quota
- 10-turn Socratic session / restart resume

说明：本地 E2E 的 AI upstream 使用 OpenAI-compatible mock，不代表真实生产模型已验证。

## 演示与恢复

- `docs/COMPETITION_DEMO_CHECKLIST.md`
- `docs/COMPETITION_RECOVERY.md`
- `scripts/pre-demo-check.ps1`
- `scripts/run-demo-smoke.ps1`

## 已知限制

- 源码包不包含长期在线的公共 Verruckt Cloud 实例。
- 真实生产 upstream model 未包含/未验证；本地 E2E 使用 mock upstream。
- Windows installer 未签名。
- Android 已完成架构适配与静态检查，尚未完成真机验证（STATIC REVIEW ONLY）。

## 目录

```text
src/                    React / TypeScript 前端
src-tauri/              Tauri + Rust Local Core
cloud/                  Verruckt Cloud 服务端 workspace
crates/                 Verruckt Vector 等共享 crate
scripts/                构建、健康检查、E2E、打包脚本
docs/                   架构、部署、比赛演示与恢复文档
deploy/                 Cloud Docker / 部署配置
public/                 静态资源
reference/              原型参考文件
docs/release/           最终 release 报告与 freeze manifest
```

## 隐私

- 学习数据默认保存在本机 SQLite。
- 登录 Cloud 后，账号、权益与同步元数据存入服务端；本地学习权威状态仍在本机。
- AI Key 只通过服务端环境变量注入，不会回传前端。
- 错误信息序列化前会做密钥脱敏。
