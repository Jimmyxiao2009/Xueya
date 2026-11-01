# COMPETITION SOURCE OF TRUTH / 竞赛事实源

> 本文档是"学涯（Orbit）"竞赛申报材料的**唯一事实来源**。
> Agent A（申报书）与 Agent B（PPT）的所有事实性陈述必须能回溯到本文档条目。
> 本文档每一条结论都经过源码/产物/测试日志核验，并标注证据路径。
>
> 核验人：事实核验 Agent（Verruckt 仓库）
> 核验时间：2026-09-20
> 代码基线：HEAD `9dcc5ac` + 工作区未提交改动（竞赛收敛改动，204 个文件）
> 测试证据基线：`audit/competition-convergence/`（2026-09-20 17:56–19:46，对应上述代码状态）

---

## 1. Naming / 命名口径

| 用途 | 名称 | 证据 |
| --- | --- | --- |
| 中文产品名 | **学涯** | `src-tauri/tauri.conf.json` productName="学涯"；窗口标题"学涯 · 学习的下一站"；`src-tauri/gen/android/app/src/main/res/values/strings.xml` app_name="学涯" |
| 英文产品名 | **Orbit** | `README.md` 标题；商店图标 `store/Orbit_*.png`；安装包文件名 `Orbit_0.1.0-rc.1_x64-setup.exe`；二进制 metadata ProductName=Orbit（`docs/RC_VALIDATION.md` I8） |
| 开发代号 | **Verruckt** | 仅用于技术标识符：applicationId `app.verruckt.desktop`、crate 名 `verruckt-*`、二进制 CompanyName=verruckt、MSIX 占位 Publisher `CN=Verruckt` |

**铁律：**

1. 面向评委的一切材料（申报书、PPT、演讲）中，产品只叫 **学涯（Orbit）**。
2. **Verruckt 是开发代号，不是产品名。** 它只能出现在"技术实现"语境（crate 名、包标识符），且首次出现必须注明"开发代号"。禁止写"Verruckt 是一款……"。
3. 品牌统一时间点：2026-09-20 约 11:50 起 UI 品牌统一为「学涯」。**早于此时间的截图页头仍显示"Orbit · 和你一起理解"，属旧品牌，不得直接使用**（见 `COMPETITION_ASSET_INDEX.md`）。
4. `Orbit` 单独出现时指英文产品名；`orbit-server` 是浏览器演示模式的 HTTP 服务组件（见 §3.3），不是另一个产品。

---

## 2. Product Definition / 产品定义

**学涯（Orbit）是以苏格拉底式连续对话为核心的本地优先 AI 学习 Agent。**

核心口径（三者必须一致，不得漂移）：

1. **Conversation 是课堂。** 点击「开始学习」后，本地核心创建 `LearningSession` 并由 Agent 主动发出开场白（opening turn），对话流是学习的主界面。证据：`src-tauri/src/services/tutor_service.rs` `opening()`（L107）；`src/components/learning/LearningScreen.tsx` 主体为消息流（L179-207）。
2. **Quiz 是教学工具和阶段检查，不是课堂本身。** 测验藏在可选的「阶段检查」开关后。证据：`LearningScreen.tsx` L226-267（`assessmentOpen` toggle，标注"阶段检查 · 可选"）。
3. **Agent 是老师，不是题目生成器。** 系统提示词定义了 13 种教学动作（teaching moves）：explain / ask / probe / hint / example / counterexample / challenge / reflect / summarize / transition / exercise / quiz / completeConcept。证据：`src-tauri/src/ai/prompts.rs` `TUTOR_V1_SYSTEM` L337。

产品闭环（README 口径，已核实与代码一致）：

```
来源 → 综合 → 路线 → 学习 → 提问 → 诊断 → 调整 → 复习
```

- 用户导入自己的资料 → 解析/分块/索引 → 生成会随作答改变的**学习路线（Roadmap）**。
- 答错或表达"我不懂" → 路线中**插入加练/复习节点**，并解释原因。证据：`audit-results/new-user-live/REPORT.md` 第 7 步（错题触发"加练 · 概率基础与分数表示"，路线 6→7 节点），数据库旁证 adaptation_events=3。
- 旧的静态课文页面已降级为可折叠的"本节讲解"（`<details className="lesson-brief">`，`LearningScreen.tsx` L164-178）。

---

## 3. Core Architecture / 核心架构

**总纲：Local-First（本地优先）。学习运行时全部在本地；云是增强，不是依赖。**

### 3.1 Local（本地核心）做什么

技术栈：Tauri 2（Rust）+ React 19 + TypeScript + Vite 8 + Tailwind CSS 4 + zustand + SQLite（sqlx）。

本地核心独占的职责（权威所有者）：

| 域 | 本地持久化（SQLite 表） |
| --- | --- |
| 学习会话 / 对话轮次 | `learning_sessions` / `learning_messages` |
| 掌握度 / 作答证据 | `mastery_states` / `learning_attempts`（另有每知识点 `learner_states`） |
| 路线 / 课程 / 自适应调整 | course / node / adaptation 表 |
| 测验与作答 | quiz / attempt 表 |
| 资料检索 / 引用 | chunks 表 + 本地向量索引 |
| AI Provider 配置与密钥 | providers 表 + 密钥库（桌面=系统 keyring；Android/服务器=AES-256-GCM 加密 vault） |

- 本地学习路径**不需要网络、不需要账号**：安装 → 跳过登录 → 建路线/导入资料 → 开始学习，全程本地（证据：`docs/RC_VALIDATION.md` 离线/故障矩阵 F1–F9 全 PASS；`audit/competition-convergence/RUNTIME_BOUNDARY.md` §2）。
- 向量引擎：`crates/verruckt-vector`（自研嵌入式引擎：exact + HNSW、WAL、墓碑删除、checksummed 磁盘格式，63 项测试）。后端可切换：`VERRUCKT_VECTOR_BACKEND=sqlite|verruckt`，默认 SQLite；逻辑在 `src-tauri/src/state.rs` L44-75。

### 3.2 Cloud（Verruckt Cloud）做什么

位置：`cloud/crates/`，共 4 个 crate：`verruckt-cloud`、`verruckt-cloud-domain`、`verruckt-cloud-protocol`、`verruckt-cloud-storage`。服务端数据库为 **PostgreSQL**。

Cloud 的职责边界（**仅限以下七类**）：

| 职责 | 模块 |
| --- | --- |
| 账号 / 身份 | `routes/auth.rs` + `routes/me.rs` |
| 设备注册 / 吊销 | `routes/devices.rs` + `services/devices.rs` |
| 权益 / 配额 | `routes/entitlements.rs` + `services/entitlements.rs` + `services/quota.rs` |
| AI 网关（代理模型调用、流式、计 token） | `routes/ai.rs` + `services/ai.rs` |
| 同步（pull/push/outbox、冲突、墓碑、重试） | `routes/sync.rs` + `services/sync.rs` |
| 文件存储（S3 兼容、预签名、去重） | `routes/files.rs` + `verruckt-cloud-storage` |
| 管理 / 健康 / 限流 | `routes/admin.rs`、`routes/health.rs`、`services/rate_limit.rs` |

**Cloud 必须不做的（已核实代码成立）**：不驱动学习会话、不当教学运行时、不决定掌握度、不远程批改主观题、不是学习内容的数据库。证据：对 `cloud/crates` 全量 grep，**零引用**本地学习服务（LearningService/TutorService 等）。

⚠️ 口径修正（编辑注）：`audit/competition-convergence/RUNTIME_BOUNDARY.md` §1 表格写掌握度存于 `learning_mastery` 表——**该表不存在**，实际表名是 `mastery_states`（`src-tauri/migrations/0001_init.sql` L151）。对外材料请使用实际表名或干脆不写表名。

### 3.3 orbit-server 是什么

- `orbit-server` 是 `src-tauri` 的一个 bin target（`web-server` feature，axum），是**真实本地核心 `AppState` 之上的 HTTP 传输层**。源码自述："HTTP transport for the real learning core; no demo seed or browser mock"（`src-tauri/src/bin/orbit-server.rs` L1）。
- 浏览器通过 `POST /api/invoke` 调用与桌面端**完全相同**的 IPC 命令集（`src-tauri/src/bin_web/dispatch.rs`）；前后端与桌面版共享同一 React 前端。
- 浏览器会话 = 服务端托管的账号工作区：每个账号独立 `AppState`、独立 SQLite、按账号隔离在 `.orbit-server/accounts/`，HttpOnly Cookie 会话。
- 用途：**Browser Demo / 开发预览**与 Docker 部署形态（`deploy/compose.yml`：postgres + cloud + web 三服务）。
- **不要**把 orbit-server 描述成"云端学习服务"或"网页版产品的全部"——它是本地核心的浏览器传输层。

---

## 4. AI / 人工智能

以下每一项均已核实真实存在于当前源码（括号为证据）：

| 概念 | 状态 | 证据 |
| --- | --- | --- |
| `LearningSession` | ✅ 存在并有独立表 `learning_sessions` | `src-tauri/src/domain/learning.rs` L15；`migrations/0002_learning_loop.sql` |
| `LearningTurn` | ✅ 存在；**运行时视图**，由 `learning_messages` 组装，无独立表 | `domain/learning.rs` L134；`services/tutor_service.rs` `turns()` L255-295 |
| `AgentAction` / `AgentActionKind` | ✅ 存在；13 个变体 | `domain/learning.rs` L104 / L47-60 |
| `LearnerResponse` / `LearnerInference` | ✅ 存在（轮次的组成类型） | `domain/learning.rs` L126 / L112 |
| `ConceptState` | ⚠️ **已定义并导出，但当前无任何代码读写、无持久化** | `domain/learning.rs` L156（全仓零使用） |
| `MasteryEvidence` | ⚠️ 同上，已定义未接线 | `domain/learning.rs` L166（全仓零使用） |
| Provider 策略 | ✅ `ChatProvider` / `EmbeddingProvider` trait；三实现：`MockProvider`、`OpenAiCompatibleProvider`（支持 `chat_completions` 与 `responses` 双协议）、`CloudAiProvider` | `ai/traits.rs` L104/L134；`ai/mock.rs` L30；`ai/openai_compatible.rs`；`ai/cloud.rs` L19 |
| 三种 AI 模式 | ✅ `AiMode::{Offline, Byok, Cloud}`；**新装默认 Offline** | `domain/provider.rs` L42-49；`ai/registry.rs` L316-328 |
| BYOK | ✅ 用户自配 OpenAI 兼容端点；未配置时**硬报错**，绝不静默降级 Mock；`first_enabled_provider` 排除 `mock://%` | `ai/registry.rs` L386-404；`db/repo/providers.rs` L24-31 |
| Cloud AI | ✅ 经 Verruckt Cloud 网关；未登录/无权益/无配额/不可用均硬报错，不回退 BYOK/Mock | `ai/cloud.rs`；`ai/registry.rs` |
| Offline / Mock 策略 | ✅ Mock 仅用于显式离线模式、测试、fixture、preview；输出明确标注，绝不冒充真实 AI | `audit/competition-convergence/AI_INTEGRATION_MATRIX.md` §1、§4 |
| `ORBIT_REQUIRE_AI=1` | ✅ 在 registry 与 CourseService 多处强制：要求真实 Provider 或显式报错 | `ai/registry.rs` L515-517；`services/course_service.rs` L138-140 |
| 掌握度证据（实际落库路径） | ✅ 对话中的 `masteryDelta` 由 `TutorService::record_conversational_evidence` 写入 `learning_attempts` + `mastery_states`（0..1 clamp） | `services/tutor_service.rs` L298 起；`db/repo/learning.rs` L117-177 |
| 苏格拉底提示词 | ✅ `TUTOR_V1_SYSTEM`（JSON 契约，含 masteryDelta）；`TUTOR_STREAM_V1_SYSTEM`（纯文本流式，**不含** masteryDelta）；`TUTOR_OPENING_V1_SYSTEM`（开场，**不含** masteryDelta） | `ai/prompts.rs` L327 / L375 / L406 |

**AI 能力的安全描述边界：**

- 可以说：13 种教学动作、开场白由 Agent 主动生成、对话证据驱动掌握度调整、错题触发路线自适应、基于本地资料的引用回答（retrieval-grounded）。
- 不要说：Agent 拥有"完整的学习者认知模型"（ConceptState 未接线）、不要说掌握度是"可视化评分"（masteryDelta 是内部证据，UI 不展示分数）、不要说所有提示词链路都产出 masteryDelta（仅 TUTOR_V1 JSON 链路）。

---

## 5. Platforms / 平台

### 5.1 Windows —— 已实现、已动态验证 ✅

| 项 | 状态 | 证据 |
| --- | --- | --- |
| NSIS 安装包 | 产物存在：`Orbit_0.1.0-rc.1_x64-setup.exe` | `.orbit-server/release/orbit-windows-0.1.0-rc.1/` |
| 安装/启动/重启/静默卸载/重装 | ✅ 实测通过（I1–I5） | `docs/RC_VALIDATION.md` |
| 数据迁移（fixture 1..5） | ✅ I6 | 同上 |
| WebView Golden Path 3×、Rust Golden Path 10× | ✅ G1/G2 | 同上；`src-tauri/tests/rc_golden_path.rs` |
| 离线/故障矩阵 F1–F9、RAG、Adaptation、宽度、A11y、密钥审计 | ✅ 全 PASS | 同上 |
| MSI / MSIX | ⚠️ 产物存在；MSIX **未签名**（Publisher 占位 `CN=Verruckt`）、自签名 smoke 通过但本机安装被策略拒绝 | `docs/STORE.md` |
| Microsoft Store 上架 | ❌ 未提交 Partner Center | `docs/STORE.md` 末尾待办 |
| ⚠️ 文档冲突 | `docs/STORE.md` 称 MSI 已构建；`docs/RELEASE_CHECKLIST.md` L37 称 MSI SKIPPED。以产物为准：MSI 文件存在，但当轮冻结流程跳过了它 | 两份文档 |

### 5.2 Android —— 仅静态适配，未真机验证 🔶

| 项 | 状态 | 证据 |
| --- | --- | --- |
| Gradle 工程 | 存在（compileSdk 36 / minSdk 24 / targetSdk 36，applicationId `app.verruckt.desktop`） | `src-tauri/gen/android/` |
| 代码级适配 | ✅ Android 专用加密密钥库（AES-256-GCM vault，替代 keyring）；向量后端强制 SQLite；禁用 window-state 插件 | `src-tauri/src/state.rs` L51-57、L97-103；`src-tauri/src/secrets/encrypted_store.rs`；`src-tauri/Cargo.toml` L69 |
| APK 产物 | ❌ 全仓库不存在任何 .apk | find 全仓无结果 |
| 真机/模拟器运行 | ❌ 无任何记录；`docs/ANDROID_DEVICE.md` 是移植指南，其 9 项 Smoke Checklist **全部未勾选** | `docs/ANDROID_DEVICE.md` L174-186；`audit-results/2026-09-20-fixes.md` L44 自承"未做 Android 真机" |
| 正确口径 | **"已完成 Android 平台的代码级适配与工程接入，真机验证待完成"** | — |

### 5.3 Browser Demo —— 已实现、已动态验证（开发环境）✅

- 用途：**竞赛演示、开发预览、Docker 部署形态**。让评委/开发者在浏览器里使用真实产品，而不是前端 mock。
- 本质：orbit-server（§3.3）= 真实本地核心的 HTTP 传输层。
- 已验证证据（2026-09-20，真实浏览器操作、真实 BYOK 模型调用、非 fixture）：
  - `audit-results/new-user-live/`（12:45–12:50）：新账号完整闭环——注册→生成 6 节点 32 题路线（229 秒）→首课学习 5 题 6 作答→**刻意答错→看纠错解释→答对**→错题触发加练节点→刷新恢复。
  - `audit-results/oobe-three-pages/`（16:03–16:12）：三页 OOBE 桌面+移动双尺寸实测。

---

## 6. Testing / 测试（当前版本可引用数字）

**唯一可信来源：`audit/competition-convergence/`（2026-09-20 17:56–19:46），对应当前代码状态。**
⚠️ README 中"cargo test：42 个单元测试 + 12 个集成测试"已过时，禁止使用。

| 套件 | 数字 | 证据文件 |
| --- | --- | --- |
| 前端 vitest | **14 文件 / 71 通过** | `test-final.txt` |
| 前端 typecheck / lint / build | 通过（build：73 模块） | `typecheck-final.txt` / `lint-after-changes.txt` / `build-after-changes.txt` |
| Rust src-tauri 单元测试（--lib） | **78 通过** | `cargo-test-lib-first-class-turns.txt` |
| learning_loop 集成 | **16 通过**（含 Agent 开场、一等轮次） | `cargo-test-learning-loop-final.txt` |
| provider_compat（BYOK 协议/不静默降级） | **10 通过** | `cargo-test-provider-require-ai.txt` |
| cloud_client（客户端↔loopback 云） | **15 通过（串行）** | `cargo-test-cloud-client-final3.txt` |
| retrieval（检索/引用） | **5 通过** | `cargo-test-retrieval.txt` |
| vector_integration（桌面向量适配） | **8 通过** | `cargo-test-vector-integration-final.txt` |
| additional_audit / astra_regressions | **6 / 12 通过** | `cargo-test-additional-audit.txt` / `cargo-test-astra-regressions.txt` |
| competition_demo / rc_golden_path | **各 1 通过** | 同名文件 |
| verruckt-vector 引擎（独立 crate） | **63 通过**（含 2 doc-test） | `vector-test-after-changes.txt` |
| Cloud 服务端单元 | **19 通过**（3+13+3 三 crate） | `audit-results/cloud-final-tests.log` |
| Cloud 集成（loopback） | **13 项不同测试**（astra 二进制 11 + health_and_error 2；其中 5 项依赖隔离 PostgreSQL，最近一次于临时容器通过） | `cloud-test-integration.txt`；`audit-results/cloud-all-integration.log` |

**不重复计数说明（给 PPT 的口径）：**

- 上述套件按**独立测试二进制**分列，彼此不重叠。例外：cloud 的 astra 二进制用 `include!` 复跑了 cloud_integration 的 5 项，已按"不同测试"去重。
- **如必须给总数：318 项不同测试通过**（71+78+74+63+19+13），另有 2 项标记 ignore 未执行。更安全的做法是分套件展示，不堆总数。
- cloud_client 有 1 项并发测试在并行下偶发失败（串行 100% 通过）——引用 cloud_client 时注明"串行执行"。

**过时数字黑名单（禁止引用）**：前端 30/32/40/63/68（历史轮次）；总计 272（`2026-09-20-fixes.md`）；桌面端 145；README 的 42+12。

---

## 7. Limitations / 真实限制（必须如实呈现）

| 限制 | 详情 | 证据 |
| --- | --- | --- |
| Live BYOK E2E 未执行 | `live_provider` 测试被 `#[ignore]`，需真实 `VERRUCKT_TEST_*` 凭据 | `cargo-test-live-provider.txt`（0 passed, 1 ignored） |
| Live Cloud E2E 未执行 | `cloud_live` 测试被 `#[ignore]`，需 Docker + `VERRUCKT_E2E_EMAIL`；云端能力验证均为 loopback | `cargo-test-cloud-live.txt` |
| 5 项 Cloud 测试依赖 PostgreSQL | 本机无 DB 时失败（环境限制，非产品回归）；最近一次通过在临时容器 | `audit-results/cloud-all-integration.log` vs `cloud-test-after-changes.txt` |
| Android 未真机 | 见 §5.2 | `docs/ANDROID_DEVICE.md` |
| Windows 安装器未在收敛轮重打包 | 现有安装包来自此前 RC 轮；MSIX 未签名、未上架 | `docs/STORE.md` |
| `ConceptState` / `MasteryEvidence` 未接线 | 类型已定义导出但零使用；掌握度实际走 `mastery_states`+`learning_attempts` | `domain/learning.rs` L156/L166 |
| `LearningTurn` 无独立表 | 是 `learning_messages` 上的运行时视图（竞赛期刻意不做 schema 迁移） | `tutor_service.rs` L255-295 |
| 确定性降级仍存在但透明 | Offline 模式下路线/课文可用确定性模板生成，provenance 标注 `deterministic`/`mock`，不冒充真实 AI | `course_service.rs` L562-600 |
| 单次全量 `cargo test` 在本机不可行 | Windows 页面文件限制，分批执行（本报告数字即分批汇总） | 收敛报告 §3.2 |
| clippy `-D warnings` / `cargo fmt --check` 未全绿 | 存在历史遗留告警与格式漂移 | 收敛报告 §3.2 |
| 已知体验问题 | 首次路线生成约 229 秒且进度停在 65%（保存阶段串行生成题目）；新建路线误出现"已为你重排本课"通知 | `audit-results/new-user-live/REPORT.md` §尚存问题 |
| 无演示录屏、无架构图 | 全仓库无 mp4/webm/mov；无任何图片形式的架构图 | 素材盘点 |
| 旧品牌截图仍在仓库 | `store/screenshots/01-03`、`.orbit-server/e2e/*` 页头为旧"Orbit"品牌 | `COMPETITION_ASSET_INDEX.md` |

---

## 8. 给 Agent A / B 的一句话口径

> 学涯（Orbit）是一个**本地优先**的 AI 学习 Agent：**对话是课堂**（Agent 用 13 种教学动作主动执教），**路线会适应你**（答错自动插入加练节点），**你的资料是上下文**（本地检索+引用）。Windows 桌面版已实测可安装运行；浏览器演示模式由同一真实核心驱动；Android 已完成代码级适配、真机验证进行中。云只做账号/同步/AI 网关，学习数据永不出本地权威。
