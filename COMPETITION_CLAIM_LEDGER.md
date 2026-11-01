# COMPETITION CLAIM LEDGER / 宣传主张核验台账

> 申报书与 PPT 中**每一句事实性宣传**必须先在这里找到条目，并使用 Safe wording。
> 验证级别：✅ 已动态验证 ｜ ✅S 已验证（静态+产物）｜ 🟡 部分验证（loopback/编译级）｜ 🔶 仅静态适配 ｜ ❌ 未验证/不可声称
> 核验基线：HEAD `9dcc5ac` + 未提交收敛改动；测试证据 `audit/competition-convergence/`（2026-09-20）。

---

## A. 平台类

### A1. "支持 Android"

- **Claim:** 学涯支持 Android。
- **Evidence:** `src-tauri/gen/android/` Gradle 工程存在（compileSdk 36/minSdk 24）；Android 专用加密密钥库 AES-256-GCM（`state.rs` L97-103）；向量后端 Android 强制 SQLite（`state.rs` L51-57）；`docs/ANDROID_DEVICE.md` 是移植指南，其 9 项 Smoke Checklist 全未勾选；全仓库无 APK；无任何真机/模拟器运行记录；`audit-results/2026-09-20-fixes.md` 自承"未做 Android 真机"。
- **Verification Level:** 🔶 仅静态适配（代码级适配完成，编译路径就绪，零动态验证）
- **Safe wording:** "已完成 Android 平台的代码级适配：移动端密钥加密、向量存储降级、Tauri Android 工程接入；真机验证正在进行中。"
- **Unsafe wording:** ❌"支持 Android 手机使用" ❌"已通过 Android 真机测试" ❌"Android 版已发布/可下载" ❌"全平台覆盖"
- **Source:** `docs/ANDROID_DEVICE.md`；`src-tauri/src/state.rs`；`audit-results/2026-09-20-fixes.md`

### A2. "Windows 桌面版可安装运行"

- **Claim:** Windows 版是真实可安装、可运行的桌面应用。
- **Evidence:** NSIS 安装包 `.orbit-server/release/orbit-windows-0.1.0-rc.1/Orbit_0.1.0-rc.1_x64-setup.exe` 存在；安装/启动/重启/静默卸载/重装实测通过（`docs/RC_VALIDATION.md` I1–I5）；数据迁移 I6；Golden Path G1/G2；SHA-256 校验记录 I9–I10。
- **Verification Level:** ✅ 已动态验证（RC 轮）；注意本轮收敛后未重新打包
- **Safe wording:** "Windows 桌面版已通过安装、启动、卸载、迁移、离线的完整 RC 验证（0.1.0-rc.1）。"
- **Unsafe wording:** ❌"已上架 Microsoft Store"（未提交 Partner Center）❌"正式签名发布"（MSIX 未签名，Publisher 为占位 CN=Verruckt）
- **Source:** `docs/RC_VALIDATION.md`；`docs/STORE.md`；`.orbit-server/release/`

### A3. "有浏览器版 / Web 版"

- **Claim:** 可以在浏览器中使用学涯。
- **Evidence:** `orbit-server`（`src-tauri/src/bin/orbit-server.rs`）是真实本地核心 AppState 上的 HTTP 传输层；浏览器会话为服务端托管的账号工作区；2026-09-20 两批真实浏览器动态验证（`audit-results/new-user-live/`、`audit-results/oobe-three-pages/`）。
- **Verification Level:** ✅ 已动态验证（开发/Docker 环境）
- **Safe wording:** "提供浏览器演示模式：由与桌面版完全相同的学习核心通过 HTTP 驱动，非前端模拟；Docker Compose 一键部署（postgres + cloud + web）。"
- **Unsafe wording:** ❌"云端 SaaS 版"（浏览器会话仍由本地核心承载）❌"公网已部署上线"
- **Source:** `src-tauri/src/bin/orbit-server.rs`；`deploy/compose.yml`；`docs/LOCAL_DEMO.md`

---

## B. 产品与 AI 类

### B1. "以苏格拉底式对话为核心，对话是课堂"

- **Claim:** 学涯以苏格拉底式连续对话为核心学习方式。
- **Evidence:** `TutorService::opening()` 在「开始学习」时由 Agent 主动开场（`tutor_service.rs` L107）；`TUTOR_V1_SYSTEM` 定义 13 种教学动作（`prompts.rs` L337）；`LearningScreen.tsx` 以对话流为主界面、课文折叠为 lesson-brief、Quiz 收进「阶段检查」toggle（L164-267）；learning_loop 16 项集成测试含开场轮次验证。
- **Verification Level:** ✅ 已动态验证
- **Safe wording:** "点击开始学习后，AI 导师主动开场；对话流是课堂，13 种教学动作（讲解/追问/提示/反例/挑战…）驱动学习；测验是可折叠的阶段检查工具。"
- **Unsafe wording:** ❌"Quiz 驱动学习" ❌"AI 自动生成题库刷题"（与产品定位相反）
- **Source:** `src-tauri/src/ai/prompts.rs`；`src/components/learning/LearningScreen.tsx`；`cargo-test-learning-loop-final.txt`

### B2. "路线自适应：答错自动插入加练节点"

- **Claim:** 学习路线会根据作答表现自适应调整。
- **Evidence:** 真实端到端记录：刻意答错骰子概率题后路线插入"加练 · 概率基础与分数表示"，路线 6→7 节点（`audit-results/new-user-live/REPORT.md` 第 7 步；`database-evidence.json` adaptation_events=3）；截图 `completed-route.png`；learning_loop 集成测试覆盖错题 adaptation 与 DB 重开持久化。
- **Verification Level:** ✅ 已动态验证（真实模型 + 自动化测试双重证据）
- **Safe wording:** "答错或表达困惑时，路线会自动插入加练/复习节点并解释原因；调整记录持久化，重启后仍在。"
- **Unsafe wording:** ❌"实时强化学习训练模型"（无模型训练，是路线结构与掌握度状态的调整）
- **Source:** `audit-results/new-user-live/`；`src-tauri/tests/learning_loop.rs`

### B3. "导入自己的资料，成为学习上下文"

- **Claim:** 用户资料可导入并成为学习/问答的上下文。
- **Evidence:** 解析→分块→索引→混合检索链路（retrieval 5 项集成测试通过）；OOBE 实测导入 3 份资料生成课程，metadata 含 3 sourceIds / 12 evidenceChunkIds、origin=byok_grounded（`audit-results/oobe-three-pages/course-evidence.json`）；导师回答可展开"依据 · …"查看真实资料片段（`docs/COMPETITION_DEMO.md` §5 第 6 步）；检索引用与系统提示词隔离（`AI_INTEGRATION_MATRIX.md` §4）。
- **Verification Level:** ✅ 已动态验证
- **Safe wording:** "导入 Markdown/文档等资料后，本地解析、分块、索引；课程内容与导师回答可带真实资料引用（'依据'可展开溯源）。"
- **Unsafe wording:** ❌"支持任意格式文件"（按实际解析器说：Markdown 已验证，docx 解析器存在于 `ingest/parser/docx.rs` 但未在演示链路实测）❌"云端资料库"（资料默认本地）
- **Source:** `cargo-test-retrieval.txt`；`audit-results/oobe-three-pages/`；`docs/COMPETITION_DEMO.md`

### B4. "掌握度追踪 / 学习状态建模"

- **Claim:** Agent 追踪学习者的掌握度。
- **Evidence:** `mastery_states` 表 + `learning_attempts` 表真实存在并读写（`migrations/0001_init.sql` L151/L141；`db/repo/learning.rs`）；对话中的 `masteryDelta` 由 `record_conversational_evidence` 落库（`tutor_service.rs` L298 起）。**但**域类型 `ConceptState`/`MasteryEvidence` 已定义导出却零使用（`domain/learning.rs` L156/L166）。
- **Verification Level:** 🟡 能力已验证（走 mastery_states/learning_attempts 路径）；名义类型未接线
- **Safe wording:** "每次对话与作答都会产生内部掌握度证据并持久化，驱动路线调整；掌握度是内部信号，不作为分数展示。"
- **Unsafe wording:** ❌"已建立 ConceptState/MasteryEvidence 证据模型并落库"（类型未接线）❌"可视化掌握度雷达图"（UI 无此物）
- **Source:** `src-tauri/src/db/repo/learning.rs`；`src-tauri/src/services/tutor_service.rs`

### B5. "BYOK：自带模型 Key"

- **Claim:** 支持用户自带 OpenAI 兼容 API Key。
- **Evidence:** `OpenAiCompatibleProvider` 同时支持 `chat_completions` 与 `responses` 两种协议，自动探测能力（`ai/openai_compatible.rs`；`provider_compat` 10 项 loopback 测试通过）；BYOK 未配置时硬报错、绝不静默回退 Mock（`registry.rs` L386-404；`providers.rs` L24-31 排除 mock://）。真实模型调用已在 OOBE/新用户实测中发生（`new-user-live` 使用 BYOK）。
- **Verification Level:** ✅ 已动态验证（loopback 协议测试 + 真实模型开发实测）；正式 live 测试 `live_provider` 被 ignore 未执行
- **Safe wording:** "BYOK 支持 OpenAI 兼容端点（chat_completions / responses 双协议，自动探测）；协议兼容性由 loopback 测试覆盖，开发环境已用真实模型完成全链路学习。"
- **Unsafe wording:** ❌"已通过全部主流厂商实测"（无此矩阵）❌"生产环境 BYOK E2E 已完成"（live_provider 测试 ignored）
- **Source:** `src-tauri/tests/provider_compat.rs`；`cargo-test-live-provider.txt`

### B6. "Cloud AI 网关"

- **Claim:** 官方云提供 AI 网关（免自备 Key）。
- **Evidence:** `cloud` 的 `routes/ai.rs`+`services/ai.rs` 实现 chat/embeddings 代理、token 计数、权益与配额门控；客户端 `cloud_client` 15 项 loopback 测试通过（含 gating、无静默回退）；live Cloud E2E（`cloud_live.rs`）被 ignore 未执行。
- **Verification Level:** 🟡 部分验证（loopback 全覆盖；live 未执行）
- **Safe wording:** "官方 Cloud AI 网关已实现并按权益/配额门控，客户端行为由 loopback 集成测试覆盖。"
- **Unsafe wording:** ❌"官方 AI 服务已上线运营" ❌"Cloud AI 端到端已实测"（live 未跑）
- **Source:** `cloud/crates/verruckt-cloud/src/routes/ai.rs`；`src-tauri/tests/cloud_client.rs`；`cargo-test-cloud-live.txt`

### B7. "云同步 / 多设备"

- **Claim:** 学习数据可云同步。
- **Evidence:** pull/push/outbox + 冲突/墓碑/重试/幂等实现；loopback 集成测试覆盖同步幂等、冲突、文件、配额（`cloud/tests/cloud_integration.rs` 5 项通过）；5 项 PostgreSQL 门控测试最近一次在临时容器通过。
- **Verification Level:** 🟡 部分验证（loopback/容器级；无真实多设备实测）
- **Safe wording:** "云同步采用本地先写 + outbox 推拉模型，幂等、冲突与墓碑语义由集成测试覆盖。"
- **Unsafe wording:** ❌"多设备无缝同步已实测"（无真实双设备记录）❌"实时协作"
- **Source:** `cloud/crates/verruckt-cloud/src/services/sync.rs`；`cloud-test-integration.txt`

### B8. "本地优先、离线可用、不登录也能学"

- **Claim:** 无需账号、无需网络即可完整学习。
- **Evidence:** 新装默认 `AiMode::Offline`（`registry.rs` L316-328）；OOBE 账号可跳过（`OnboardingGate.tsx` L85-88）；离线/故障矩阵 F1–F9 实测 PASS（`RC_VALIDATION.md`）；`RUNTIME_BOUNDARY.md` §2 本地路径。
- **Verification Level:** ✅ 已动态验证
- **Safe wording:** "安装后跳过登录即可学习；断网时路线、检索、引用、错题自适应全部可用；账号只用于云同步与官方 AI。"
- **Unsafe wording:** ❌"完全不需要服务器"（浏览器演示模式与云功能需要）——注意限定语场景
- **Source:** `docs/RC_VALIDATION.md`；`src/components/onboarding/OnboardingGate.tsx`

### B9. "Mock 不冒充真实 AI"

- **Claim:** 离线 Mock 输出明确标注，绝不伪装成真实 AI。
- **Evidence:** provenance 标签体系（`byok_*`/`official_*`/`mock`/`deterministic`）；`ORBIT_REQUIRE_AI=1` 在 registry 强制真实 Provider 或报错；`provider_compat` 含"byok_mode_without_provider_never_falls_back_to_mock"测试。
- **Verification Level:** ✅ 已动态验证（自动化测试）
- **Safe wording:** "AI 输出带来源标签；离线模式输出明确标注为确定性内容；生产可设 ORBIT_REQUIRE_AI=1 强制真实模型。"
- **Unsafe wording:** 无（这是防守型优势，如实讲即可）
- **Source:** `audit/competition-convergence/AI_INTEGRATION_MATRIX.md`；`src-tauri/src/ai/registry.rs`

### B10. "多 Agent 内容生成"

- **Claim:** 课程内容由多 Agent 协作生成。
- **Evidence:** `docs/MULTI_AGENT_GENERATION.md`；2026-09-20 15:45 真实模型实测（`audit-results/multi-agent/live-route.png` 显示逐节点"正在准备讲解与练习"状态；`live-evidence.json` 节点级证据）。
- **Verification Level:** ✅ 已动态验证（开发环境真实模型）
- **Safe wording:** "路线生成采用大纲 Agent + 节点内容任务的多 Agent 流水线，每个节点的准备状态对用户可见。"
- **Unsafe wording:** ❌"生成速度极快"（实测首次 6 节点约 229 秒）❌"100% 成功"（实测含"内容生成失败，打开后重试"节点）
- **Source:** `audit-results/multi-agent/`；`audit-results/new-user-live/REPORT.md` §尚存问题

---

## C. 测试与质量数据类

### C1. "XXX 项自动化测试"

- **Claim:** 项目有 N 项自动化测试。
- **Evidence:** 见 `COMPETITION_SOURCE_OF_TRUTH.md` §6 完整表：前端 71、Rust 单元 78、src-tauri 集成 74（7 个二进制）、向量引擎 63、Cloud 单元 19、Cloud 集成 13（不同测试）。
- **Verification Level:** ✅ 已动态验证（2026-09-20 日志）
- **Safe wording:** 分套件列出；如需总数："**318 项不同的自动化测试通过**（另有 2 项 live 测试标记为需真实凭据未执行）"，且必须能说出构成。
- **Unsafe wording:** ❌README 的"42+12"（过时）❌"272 项"（2026-09-20 凌晨旧口径）❌任何把 cloud astra 二进制与 cloud_integration 重复相加的数字（如"cloud 18 项"）
- **Source:** `audit/competition-convergence/`（test-final.txt 等）

### C2. "测试全部通过 / 全绿"

- **Claim:** 所有测试通过。
- **Evidence:** 引用范围内属实，但存在限定：cloud_client 1 项并发测试并行下偶发失败（串行通过）；5 项 Cloud 测试需隔离 PostgreSQL；2 项 live 测试 ignored；clippy -D warnings 与 cargo fmt --check 未全绿。
- **Verification Level:** 🟡 部分真实
- **Safe wording:** "列出的 318 项测试全部通过；并发场景与 PostgreSQL 门控项的执行条件已在文档注明。"
- **Unsafe wording:** ❌"全部测试无条件通过" ❌"CI 全绿"（无 CI 证据）
- **Source:** `cargo-test-cloud-client-final3.txt`；收敛报告 §3.2/§4.3

### C3. "自研向量引擎"

- **Claim:** 内置自研嵌入式向量引擎。
- **Evidence:** `crates/verruckt-vector`（exact+HNSW、WAL、墓碑、compaction、checksummed 格式），63 项测试通过；桌面适配 8 项集成测试；Android 上强制 SQLite 后端（`state.rs` L51-57）。
- **Verification Level:** ✅ 已动态验证
- **Safe wording:** "自研嵌入式向量引擎（HNSW + WAL 持久化，63 项引擎级测试）；桌面默认可选用，移动端自动降级 SQLite 后端。"
- **Unsafe wording:** ❌"向量引擎已在 Android 实测" ❌"性能碾压 Faiss"（无对比基准报告进 PPT）
- **Source:** `crates/verruckt-vector/`；`vector-test-after-changes.txt`

---

## D. 命名与身份类

### D1. 产品名使用

- **Claim:** 产品叫 学涯 / Orbit / Verruckt。
- **Evidence:** 中文名"学涯"（tauri.conf.json productName、窗口标题、Android strings）；英文名 Orbit（README、商店图标、安装包文件名、二进制 metadata）；Verruckt 仅出现在技术标识符（app.verruckt.desktop、verruckt-* crate、CompanyName）。
- **Verification Level:** ✅S 静态核验
- **Safe wording:** "学涯（英文名 Orbit，开发代号 Verruckt）"，之后中文材料统一用"学涯"。
- **Unsafe wording:** ❌"Verruckt 学习平台"（代号当产品名）❌同一页 PPT 混用"学涯"和"Orbit"指代却不说明关系 ❌把 orbit-server 写成"Orbit 云"
- **Source:** `src-tauri/tauri.conf.json`；`store/`；`docs/RC_VALIDATION.md` I8

### D2. "开源 / 自研比例"

- **Claim:**（若出现）核心模块自研。
- **Evidence:** 向量引擎、同步协议、教学提示词体系、IPC 层均为仓库内自研；依赖 Tauri/React/sqlx/axum 等开源框架。
- **Verification Level:** ✅S 静态核验
- **Safe wording:** "学习闭环、向量引擎、同步协议、教学 Agent 提示词体系为自研；基于 Tauri 2 + React 19 开源栈构建。"
- **Unsafe wording:** ❌"完全自研无依赖" ❌"自研大模型"（模型来自 BYOK/Cloud 接入）
- **Source:** 仓库结构

---

## E. 一票否决项（出现在任何材料中即打回）

1. Android 已真机验证 / 已上架应用商店（任何商店）。
2. Cloud 是学习运行时 / 学习数据存在云端权威库。
3. Quiz 是核心玩法 / 刷题 App 定位。
4. 测试数字引用 README 的 42+12 或旧口径 272。
5. Verruckt 作为产品名面向评委。
6. "完全离线"与"Cloud AI 已实测"同时出现（互斥口径，见 `docs/COMPETITION_DEMO.md` §2）。
7. ConceptState/MasteryEvidence 被描述为"已落库的数据模型"。
8. 任何无证据的性能对比数字（速度、准确率、召回率百分比）。
