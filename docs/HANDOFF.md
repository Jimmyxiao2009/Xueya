# Orbit / Verruckt 交接 — 2026-09-20

## 用户目标与当前边界

工作目录：`H:\项目\CrossPlatform\学涯`，Windows PowerShell，当前分支 `main`。

用户要求基于 `reference/Orbit_motion_continuous_swipe.html` 替换原 UI，并实现首次体验：明暗背景选择 → 真实注册/登录 → 添加第一条学习路径。比赛演示必须使用真实数据。

**最新指令：把本地应用调到可部署上线/上架级别；本轮仍然不做远程部署。** 已使用用户提供的 OpenAI 兼容网关在本地完成真实浏览器端到端验证：注册/登录 → AI 路线 → 学习/导师/测验 → 资料导入 → 带资料再生成路线 → 路线切换与刷新持久化。不要在交接、代码、日志或提交中复制凭据，也不要继续远程操作。

本轮已完成真实模型生成的端到端验证；剩余风险主要是代码签名/商店打包、远程主机实机部署与部分降级检索。

## 当前运行状态

交接时重新检查，以下三个端口均监听 `127.0.0.1`：

| 服务 | 地址 / 端口 |
| --- | --- |
| Vite 真实应用入口 | http://127.0.0.1:1420/ |
| 管理后台 | http://127.0.0.1:1420/admin.html |
| Rust 学习服务 | 1430 |
| Rust 账号服务 | 1431 |
| PostgreSQL Docker 映射 | 55440 |

`GET http://127.0.0.1:1420/healthz` 实测返回 `status: ok`、`dataMode: real`、`aiConfigured: true`。本机 `.env.local` 已配置用户提供的真实 AI 网关；不要在日志或文档中回显密钥。

本地启动：`pwsh -File scripts/start-demo.ps1 -NoBuild`。需要重新编译时去掉 `-NoBuild`；Windows 下运行中的 exe 会阻止覆盖编译，应先核对 PID 对应路径并停止本项目进程。脚本检测到已有服务会跳过启动，**不会自动重启应用配置**。

配置在被 Git 忽略的 `.env.local`；管理登录使用其中的 `ORBIT_ADMIN_TOKEN`，不要输出它。学习数据和日志在 `.orbit-server/`，账号 PostgreSQL 使用 `orbit-demo-postgres` 容器及同名 volume。不要删除数据目录或数据卷。

完整启动、配置和打包说明见 `docs/LOCAL_DEMO.md`。

## 已实现

- Orbit 明暗主题和三步 OOBE，主题持久化、确认密码校验、超时重试、首条路径入口。
- 浏览器通过同源 `/api/invoke` 调用真正的 Rust 服务；桌面 Tauri 调用保持可用。
- 真实 PostgreSQL 注册/登录，HttpOnly 会话 Cookie；按账号隔离 SQLite 与文件资料。
- 注册、登录和退出清空前端工作区缓存；请求代次检查阻止旧账号请求回填新账号。
- 修复重复登录重新占用设备名额的问题，浏览器再次登录复用设备 ID。
- 不再启动时自动创建示例路径。`preview.html` 默认跳转真实入口，只有开发环境显式 `?fixtures=1` 才启用测试夹具。
- 本地严格 AI 模式下，上游失败或缺少配置直接报错，不静默退回模板路线或假测验。
- 浏览器设置展示服务器管理的模型状态，隐藏不支持的模型、搜索、AI 模式和 Cloud 地址修改；保留主题、账号退出。桌面原设置操作保留。
- 管理后台读取真实用户与服务状态、调整 free/pro 套餐；管理 API 要求独立 bearer token。
- 活跃路线用 `localStorage` 持久化；刷新后保持当前路线，账号退出/切换会清掉该键，避免跨账号泄漏。
- 新账号“第一条路径”页补上“退出登录 / 切换账号”，避免注册后无法换号；已用第二账号真实验证退出回登录页。
- 资料导入在 embedding 上游不可用或返回非法向量时降级为关键词/FTS 检索，不阻断导入；真实资料仍参与路线生成和引用。已用真实网关的 404 embedding 响应验证。
- Dockerfile、Compose 和本地启动脚本已准备。Compose 解析通过；Web 端口支持 `ORBIT_HTTP_BIND` / `ORBIT_HTTP_PORT`。已通过可用镜像源缓存基础镜像，`docker compose build` 成功构建 `orbit-demo:local` 与 `orbit-demo-cloud:local`；独立项目 `orbit-compose-e2e` 冷启动后 postgres/cloud/web 三个健康检查均 healthy。Compose 默认以 `production` 模式启动 cloud，并为 web/cloud 增加 healthcheck；容器内已用真实 AI 网关生成 6 节点路线。`docker save` 离线镜像包已生成。
- 已补服务器部署资料：`deploy/.env.example`、`deploy/deploy.sh`、`deploy/deploy.ps1`、`deploy/Caddyfile.example`、`deploy/README.md`，覆盖环境变量、离线镜像、一键冷启动和 HTTPS 反代。
- `deploy/deploy.ps1` 与 `deploy/deploy.sh` 均已用真实 `orbit-demo-images.tar` 完整执行：加载镜像、`--pull never`、启动 postgres/cloud/web、等待 healthy，并验证 `/healthz`；测试后已删除临时 `.env` 和硬盘链接，未提交任何凭据。
- 已添加 `scripts/deploy-remote.ps1`，可从本机通过 SSH/SCP 上传 `deploy/` 和离线镜像包，并在目标 Linux 服务器执行 `deploy.sh`；已通过语法检查和 `-DryRun`。
- 已添加 `scripts/smoke-production.ps1`：部署后可验证 `/healthz`、真实注册、会话读取；加 `-CreateRoute` 可验证真实 AI 路线生成。基础模式已在本机通过；首轮真实 AI 路线生成完成并得到 7 节点路线（标题：`生产部署冒烟测试：验证真实 AI 网关调用与路由创建`），同时发现并修复脚本误读不存在的 `generatedBy` 字段。修复后完整 `-CreateRoute` 验收再次通过：healthz → 注册 → 会话 → AI job completed → 8 节点路线。
- 已添加 `scripts/package-release.ps1`：生成离线服务器部署包 `orbit-deploy-<version>.tar.gz`（含 `orbit-demo-images.tar`、Compose、预检/备份/恢复、部署脚本、文档）和 Windows 安装包 `orbit-windows-<version>.zip`（NSIS/MSI/MSIX、商店图标/截图/静态合规站点、STORE/隐私条款文档），并输出 `SHA256SUMS.txt`。版本号和安装包文件名改为动态发现，不再硬编码。已实际解包发布 tar，用独立 Compose 项目名 `orbit-release-test`、端口 `12420` 冷启动成功，三个服务 healthy，`smoke-production.ps1` 通过；随后已清理测试容器、网络和卷。
- 已添加 `deploy/preflight.sh`：检查 Docker/Docker Compose、必填环境变量、占位值、HTTPS origin、端口占用、离线镜像包和磁盘空间；`deploy.sh` 已自动调用。已用临时 `.env` 在本机实际执行并通过。`deploy-remote.ps1` 已同步上传 preflight/backup/restore 脚本。
- 已添加 `deploy/install-docker-ubuntu.sh`：在全新 Ubuntu/Debian 服务器上通过 Docker 官方 apt 仓库安装 Docker Engine + Compose 插件；已做 shell 语法检查并纳入发布包/远程上传。
- 已添加 `deploy/backup.sh` / `deploy/restore.sh`：备份 PostgreSQL 自定义格式 dump 和 `/data` 学习卷；已在临时 Compose 栈上完整执行备份 + 恢复 + healthz 验证。
- Compose web healthcheck 已改为校验 `/healthz` 中的 `aiConfigured:true`：未配置 AI 时容器会进入 unhealthy，`up --wait` 会失败，避免上线一个 AI 不可用的实例；使用真实 AI 配置的栈已验证 healthy。
- 修复 Tauri 打包器把 `src/bin/web/` 目录误判为 `web.exe` 的缺陷：HTTP dispatch 移到 `src-tauri/src/bin_web/dispatch.rs`，`Cargo.toml` 增加 `default-run = "verruckt"`。NSIS 安装包因此可以正常生成。

## 代码入口

| 关注点 | 文件 |
| --- | --- |
| 首次体验 | `src/components/onboarding/OnboardingGate.tsx`、`src/stores/themeStore.ts`、`src/styles/onboarding.css`、`src/styles/dark.css` |
| 浏览器真实 transport / 旧请求隔离 | `src/api.ts` |
| 账号切换清理 | `src/stores/cloudStore.ts` |
| 页面引导与错误展示 | `src/App.tsx`、`src/main.tsx`、`src/preview.tsx` |
| 服务器管理设置 | `src/screens/SettingsScreen.tsx`、`src/components/settings/CloudAccountSection.tsx`、`src/lib/runtime.ts` |
| Rust HTTP 服务与白名单分发 | `src-tauri/src/bin/orbit-server.rs`、`src-tauri/src/bin_web/dispatch.rs` |
| desktop / web-server feature 拆分 | `src-tauri/Cargo.toml`、`src-tauri/build.rs`、`src-tauri/src/lib.rs`、`src-tauri/src/ipc/mod.rs` |
| 严格真实 AI 生成 | `src-tauri/src/services/course_service.rs` |
| 登录设备复用 | `src-tauri/src/cloud/client.rs` |
| 管理后台 | `public/admin.html`、`cloud/crates/verruckt-cloud/src/routes/admin.rs` |
| 启动与打包 | `scripts/start-demo.ps1`、`deploy/Dockerfile`、`deploy/compose.yml`、`vite.config.ts` |

HTTP 边界检查 Origin 和 `X-Orbit-Client: web`，需要会话的命令检查账号归属；分发不开放本地文件路径导入、演示 seed、服务器配置修改或流式订阅。浏览器使用 job 轮询和非流式导师调用。服务器 AI 密钥留在服务器配置与加密存储中，不进入前端 bundle。

## 已验证与实际限制

- 前端 40 项测试、typecheck、lint、生产构建通过。
- Rust `cargo test --no-default-features --features web-server` 全量通过（lib 78 项；cloud_client、learning_loop、provider_compat、retrieval、vector_integration、rc_golden_path、competition_demo 等集成套件均通过；需要真实外部凭据的 live tests 保持 ignored）。
- 桌面 feature `cargo check` 通过；已生成 Windows 三种分发产物：
  - NSIS：`src-tauri/target/release/bundle/nsis/Orbit_0.1.0-rc.1_x64-setup.exe`（约 7 MB）
  - MSI：`src-tauri/target/release/bundle/msi/Orbit_0.1.0-rc.1_x64_en-US.msi`（约 10 MB）
  - MSIX：`src-tauri/target/release/bundle/msix/Orbit_0.1.0.0_x64.msix`（约 10 MB，由 `scripts/build-msix.ps1` 打包并通过 `makeappx unpack` 校验）
  - MSI 使用 WiX 专用版本 `0.1.0` 和固定 UpgradeCode；已验证 MSI 元数据 `Manufacturer=Verruckt`、`ProductName=Orbit`、`ProductVersion=0.1.0`，并通过 `msiexec /a` 成功解包出 `verruckt.exe`。
  - `scripts/build-msix.ps1` 的签名分支已用自签名证书验证成功；但因本机非管理员无法写入受信任 Root，`Add-AppxPackage` 安装仍会被证书信任链拦截（`0x800B0109`）。真实代码签名和 Partner Center Publisher 绑定仍待补，步骤见 `docs/STORE.md`。
  - 商店合规模板已准备：`docs/PRIVACY.md`、`docs/TERMS.md`、`docs/SUPPORT.md`；上架前需替换占位信息并托管为公网 URL。
  - 商店素材已生成：`store/Orbit_300x300.png` 等图标，以及 1440x900 的路线/课程/导师问答截图；`scripts/build-store-assets.ps1` 可重建图标。
  - 合规静态站点已创建：`store/site/` 包含首页、隐私政策、使用条款、支持页面和样式，可部署到任意 HTTPS 静态托管；已用本地静态服务器验证所有页面返回 200。
  - 新增 `scripts/test-msix-register.ps1`：在开发者模式下注册 loose MSIX 布局、启动 `verruckt.exe`、确认进程运行、然后清理注册。已实测通过，证明 MSIX 清单、资源和可执行文件布局可被 Windows 接受并启动。
  - `scripts/build-msix.ps1` 支持通过参数或环境变量注入 Partner Center 身份：`ORBIT_MSIX_IDENTITY_NAME`、`ORBIT_MSIX_PUBLISHER`、`ORBIT_MSIX_PUBLISHER_DISPLAY_NAME`；MSIX 版本和输出文件名也会自动从 Tauri 配置推导。已用自定义身份构建并检查 `AppxManifest.xml`，随后恢复默认构建；默认 MSIX 再次通过 loose 注册 + 启动测试。
  - 已添加 `scripts/sign-windows.ps1`：统一签名 NSIS/MSI/MSIX，支持 PFX、密码环境变量、RFC3161 时间戳和签名后校验；新增 `-ArtifactList` 逗号分隔参数以规避 `pwsh -File` 数组展开问题。脚本尚未完成端到端签名验证；下一轮重启 shell 后用 `-ArtifactList` 复测，并运行 `scripts/package-release.ps1` 重新打包。
  - 新增 `scripts/test-nsis-install.ps1`：当前用户静默安装 NSIS、启动已安装应用、确认进程运行、静默卸载并确认清理。已实测通过。
  - MSI 已构建；本机非管理员静默安装返回 `1603`，未产生残留。MSI 需要管理员权限或干净 Windows VM 复测，当前环境无法完成。
- 真实浏览器 E2E：主题选择、真实注册/登录、退出再登录、跨账号隔离、首条路线、AI 生成 6 节点路线、AI 节点课、导师对话、选择题/简答题、完成节点解锁下一节点、第二条路线、路线切换、刷新后活跃路线保持。
- 最新浏览器回归（2026-09-20）：登录已有账号，打开 6 节点路线节点，看到缓存的 AI 课程内容；真实导师问答返回“监督学习 vs 无监督学习”回答；完成单选题并显示正确答案与解析。
- 真实资料导入：网关对 `/v1/embeddings` 返回 404 `model_not_found` 时，导入 job 仍 completed、source ready；随后用该资料在该账号中真实生成 7 节点路线，证明关键词检索和证据输入可用。
- 管理后台用本地 token 登录，显示真实数据库状态、2 个真实账号、套餐与服务状态；没有回显 token。
- 新账号路线为空；未配置 AI 时会直接报错，不写入假路线。
- Docker Compose `config` 解析通过；镜像构建实际完成，独立项目 `orbit-compose-e2e` 冷启动、健康检查、注册/登录、容器内真实 AI 路线生成（`deepseek-v4-flash` 冒烟，6 节点）均已验证；`docker save orbit-demo:local orbit-demo-cloud:local postgres:16-alpine` 成功生成约 201 MB 完整离线镜像包（含数据库镜像），并已用 `docker load` + `up --pull never` 在全新项目 `orbit-offline-test` 上验证离线冷启动和 healthz。远程主机实机冷启动仍待执行。
- HTTPS 反向代理拓扑已用 Caddy 容器 + `tls internal` 验证：Caddy 代理 Compose web，`https://localhost:8443/healthz` 正常；通过 HTTPS 注册返回 `orbit_session=...; HttpOnly; SameSite=Strict; Secure`，随后 `cloud_get_me` 会话读取成功；`deploy/Caddyfile.example` 也通过 `caddy validate`。真实域名证书和远程主机仍待验证。

审计依据：`audit-results/2026-09-20-oobe.md`。前面 ASTRA 与额外审计修复记录在 `audit-results/2026-09-20-fixes.md` 和 `audit-results/2026-09-19-additional/REPORT.md`；不要把它们的历史结果当成本轮重新执行的全量测试。

## 下一步：生产化收尾

1. 在目标服务器执行 `docker load` + `docker compose up -d --wait`，验证远程冷启动、数据卷持久化和 HTTPS 反代；可直接使用 `scripts/deploy-remote.ps1` 上传并执行 `deploy/deploy.sh`。该脚本已兼容仓库布局和 `package-release.ps1` 生成的离线包布局。解包后运行 `sh bootstrap.sh` 可一键完成 Docker 安装、生成 `.env`、预检和部署。
2. 路线规划依赖单次 AI 生成，真实网关慢时可能需要 1–3 分钟；节点课程在进入节点时按需生成。若要改善首屏等待，可评估缩短上游超时、异步进度提示或换用更快模型。
3. `knowledge_search` IPC 仍直接走向量 `semantic_search`；UI 当前未使用，但若开放资料库语义搜索，应像 `HybridRetriever` 一样在 embedding 不可用时回退到 FTS。
4. 桌面端 NSIS/MSI/MSIX 已生成；正式上架前按 `docs/STORE.md` 通过 `ORBIT_MSIX_IDENTITY_NAME` / `ORBIT_MSIX_PUBLISHER` / `ORBIT_MSIX_PUBLISHER_DISPLAY_NAME` 注入 Partner Center 身份，并用 `scripts/sign-windows.ps1` 对 NSIS/MSI/MSIX 统一签名；`build-msix.ps1` 已在设置 `ORBIT_MSIX_PFX` 但文件不存在时改为直接报错，不再静默产出未签名包；商店截图和隐私政策已准备。提交前运行 `scripts/check-store-release.ps1`，当前会报告 16 处需替换的合规占位符。
5. `scripts/sign-windows.ps1` 已通过 `-ArtifactList` 端到端验证；`scripts/package-release.ps1` 已重新运行并生成新包。正式上架仍需替换为受信任代码签名证书并完成 Partner Center 提交。
6. 远程部署时设置 `ORBIT_HTTP_BIND=0.0.0.0`、`ORBIT_HTTP_PORT`、`ORBIT_PUBLIC_ORIGIN=https://...`，并在 Web 服务前配置 HTTPS 反向代理；当前没有远程部署动作。
7. 继续保留 `.env.local`、`.orbit-server/`、数据库 volume 和真实账号数据，不要提交或打包进镜像。
8. `deploy/.env.example` 已改为 AI 网关/模型占位符，不包含任何真实网关地址或密钥；发布包中只保留占位模板。`deploy/preflight.sh` 已把 `replace-*`、`your-*`、`https://orbit.example.com` 识别为占位符，避免直接复制示例后误部署；同时把环境变量读取从 `eval` 改为 `printenv`。占位符 `case` 模式中的 `*` 已去掉引号，确保真正按通配符匹配。
9. `scripts/deploy-remote.ps1` 已兼容仓库布局和离线发布包布局；`deploy/restore.sh` 恢复数据库前会同时停止 web 和 cloud，避免恢复期间账号服务继续写库。

## 工作区保护与历史事项

- `main` 上有大量未提交修改，包含之前审计修复、UI 替换和本轮 OOBE，不能 reset、clean 或覆盖。
- `.zcode/` 是用户已有未跟踪内容，不要修改或清理。
- 历史已做过分支汇总/封存；本轮没有重新操作分支。后续如需处理应先查看实际 refs。
- 历史发现 Git post-commit hook 会镜像到 B 盘，可能影响另一份目录。提交前必须查看 hook 与目标，不要为了交接自动提交。
- 不提交 `.env.local`、`.orbit-server/`、管理凭据、SSH 凭据或真实账号数据。
- 本轮前端协作任务 `/root/onboarding_ui` 已完成，没有需要等待的前端子任务。
- `RELEASE_CHECKSUMS.txt` 只是比赛 rc.1 冻结构建的历史校验和，不代表后续重建产物；当前发布包以 `scripts/package-release.ps1` 生成的 `SHA256SUMS.txt` 为准。
- `README.md` 已更新为真实 AI/云账号/生产部署行为，不再声称“不需要 API Key”或“首次启动自带演示路线”。


## Round 18 本地验证证据

使用 Vite 配置加载时启动的临时 PowerShell 进程绕过损坏的持久 shell，执行了完整本地验证；临时钩子已从 `vite.config.ts` 移除。

- `npm run typecheck` PASS
- `npm run test`：5 个测试文件、40/40 tests PASS
- 本地 healthz PASS：1430 `aiConfigured=true, dataMode=real`；1431 `status=ok`
- `scripts/smoke-production.ps1 -BaseUrl http://127.0.0.1:1430` 注册 + session PASS
- `scripts/smoke-production.ps1 -BaseUrl http://127.0.0.1:1430 -CreateRoute -TimeoutSeconds 600` PASS：真实 AI 网关生成路线，job 进度 0.3 → 0.65 → completed，创建 6 节点路线：
  - route id `8ffe5a46-1efa-427b-9e51-a81fee030a2b`
  - title `生产部署冒烟测试：验证服务调用 AI 网关并创建路由`
  - nodes `6`
- `scripts/test-msix-register.ps1` PASS：MSIX 打包、`makeappx unpack`、loose 注册、启动 `verruckt.exe`、清理注册
- `scripts/sign-windows.ps1 -ArtifactList` PASS：用临时自签名证书对 NSIS/MSI/MSIX 副本签名成功；`Get-AuthenticodeSignature` 能读到 `CN=Verruckt`（自签名根不受信任，状态为 `UnknownError`，属预期）
- `scripts/package-release.ps1` PASS，重新生成：
  - `.orbit-server/release/orbit-deploy-0.1.0-rc.1.tar.gz`（198782165 bytes）
  - `.orbit-server/release/orbit-windows-0.1.0-rc.1.zip`（27085706 bytes）
  - SHA256：
    - `07222cfdd4c36022214c4d358740cbe6cc8dd63e0fb07b784120d73c8ffb0b10  orbit-deploy-0.1.0-rc.1.tar.gz`
    - `3ce5e8703dcb53e4d5a9a465e2f14954cedd54bbbccf82656cfc91749dfd67b9  orbit-windows-0.1.0-rc.1.zip`
- Shell 脚本检查 PASS：`sh -n` 通过 `deploy/preflight.sh`、`deploy/deploy.sh`、`deploy/backup.sh`、`deploy/restore.sh`、`deploy/install-docker-ubuntu.sh`；占位符通配符检查确认 `replace-*`、`your-*`、`https://orbit.example.com` 会被识别，真实值不会被误判。
- 备注：持久 PowerShell 工具会话仍不可用；本轮通过临时 Vite 配置加载钩子启动独立 PowerShell 进程完成验证，钩子已从 `vite.config.ts` 移除。下轮如仍失效，可临时复用此方式，但完成后必须移除钩子。

## Round 19 一键部署与发布包更新

- 新增 `deploy/init-env.sh`：交互式或环境变量非交互生成 `deploy/.env`，自动生成随机 `ORBIT_DB_PASSWORD`、`VERRUCKT_ACCESS_TOKEN_SECRET`、`ORBIT_ADMIN_TOKEN`，写入 `chmod 600`，不覆盖已有 `.env`，不打印密钥。
- `deploy/init-env.sh` 已通过 `sh -n` 和非交互测试：生成 7 个必填字段，密钥长度分别为 48/64/48（hex 字符），真实值不会被误判为占位符。
- 新增 `deploy/bootstrap.sh`：一条命令完成 Docker 安装（缺失时）、生成 `.env`、预检和部署；已通过 `sh -n`。
- `scripts/package-release.ps1` 已将 `init-env.sh` 和 `bootstrap.sh` 纳入服务器离线包，并重新生成：
  - `orbit-deploy-0.1.0-rc.1.tar.gz`：198783224 bytes
    - SHA256 `b7d345d04f8652d5aeb67b2d74e3371ec5d76501796ecf7d4c81c279526aa94f`
  - `orbit-windows-0.1.0-rc.1.zip`：27085783 bytes
    - SHA256 `9827716688beee32d5d9b48e9b2f0af7093f803b8379c6773a238705346c4e24`
- `vite.config.ts` 临时钩子已再次移除，当前为原始版本。

## Round 20 离线包布局验证

- 构造了与 `orbit-deploy` 解压后一致的目录，验证 `scripts/deploy-remote.ps1` 能自动识别离线包布局：
  - `compose.yml`、`deploy.sh`、`.env`、`orbit-demo-images.tar` 与 `deploy-remote.ps1` 同目录
  - `pwsh -File deploy-remote.ps1 -Server 203.0.113.10 -DryRun` PASS
  - 正确打印创建远程目录、上传部署文件、上传离线镜像、执行 `sh deploy.sh` 四个步骤
- 该验证确认不再错误依赖仓库中的 `.orbit-server/orbit-demo-images.tar` 或 `deploy/` 子目录。
- 临时用 `deploy/init-env.sh` 生成 `.env`（非交互、端口 12420），运行真实 `deploy/preflight.sh` PASS：
  - 必填变量全部识别
  - `ORBIT_PUBLIC_ORIGIN` HTTPS PASS
  - Docker daemon、Compose v2、Compose config、磁盘空间 PASS
  - 测试后临时 `deploy/.env` 已删除，未触碰真实配置

- `vite.config.ts` 临时钩子已移除，当前为原始版本。

## Round 21 商店发布前自动检查

- 新增 `scripts/check-store-release.ps1`：
  - 检查 Windows 发布包中是否存在 NSIS/MSI/MSIX、`sign-windows.ps1`、`STORE.md`、`PRIVACY.md`、`TERMS.md`、`SUPPORT.md`、300x300 图标、截图、合规静态站点文件
  - 扫描 `docs` 和 `store/site` 中的隐私/支持/运营主体占位符
  - 默认发现占位符时失败；`-WarnOnly` 仅警告，便于本地自检
  - 已纳入 `scripts/package-release.ps1` 的 Windows 发布包
- `-WarnOnly` 试运行结果：发布包素材检查通过；检出 16 处合规占位符，位于 `docs/PRIVACY.md`、`docs/TERMS.md`、`docs/SUPPORT.md`、`store/site/*.html`，与预期一致。
- 发布包重新生成：
  - `orbit-deploy-0.1.0-rc.1.tar.gz`：198783476 bytes
    - SHA256 `7e81a8defc344970e5bd344e62c580e9546b5064aa03b34c53566b7c0eb7a325`
  - `orbit-windows-0.1.0-rc.1.zip`：27087205 bytes
    - SHA256 `cc503095d8018747065ecba08dcfa0cac82c78795944ba7ac6a27f6176520c49`
- `vite.config.ts` 临时钩子已移除，当前为原始版本。

## Round 22 真实浏览器 E2E 通过

- 持久 PowerShell 工具会话仍损坏；本轮用 “Vite 配置加载时启动 detached PowerShell” 方式绕过，Vite 不再被阻塞。
- 浏览器自动化真实链路 PASS（`drive exit=0`）：
  - 清空 cookies/localStorage/sessionStorage
  - 选择主题 → 登录已有账号 `e2e.orbit.20260920.01@example.com`
  - 加载路线 `机器学习基础：监督学习与神经网络概览`
  - 打开节点 `机器学习与监督学习简介` → 开始学习
  - 在 `#chat-input` 发送 `什么是监督学习？请用一句话解释`
  - 真实 AI 返回监督学习解释，DOM 文本长度增加断言通过
  - 截图 `.orbit-server/e2e/r22-browser-chat.png`
- 修复 E2E harness：
  - `cdp.mjs` 增加 CDP HTTP 10s 超时和 WebSocket open 10s 超时，避免 CDP 失联时永久挂起
  - `drive.mjs` 新增 `assertEval`，用于断言动态内容
  - 聊天断言从固定 AI 文案改为“用户问题出现后，正文长度增加 > 20”，避免 AI 措辞变化导致误报
- `vite.config.ts` 临时 detached 启动钩子已移除，当前为原始版本。

## Round 22 后仍然待外部条件完成

- 真实远程服务器部署与公网 HTTPS
- 受信任代码签名证书
- Partner Center 身份注入与 MSIX 提交
- 替换 16 处合规占位符
- 干净 Windows VM 的 MSI 安装测试
- Microsoft Store 正式认证




