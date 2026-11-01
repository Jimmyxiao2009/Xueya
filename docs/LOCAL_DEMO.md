# Orbit 本地真实数据演示

首次启动：`pwsh -File scripts/start-demo.ps1`。需要 Docker Desktop、Rust、Node.js。已编译后加 `-NoBuild`。

- 页面：http://127.0.0.1:1420/ （不要使用 preview.html）
- 管理：http://127.0.0.1:1420/admin.html
- 管理凭据：本机 `.env.local` 中 `ORBIT_ADMIN_TOKEN`，仅在后台登录框输入。
- PostgreSQL：仅监听本机 55440；账号服务 1431；学习引擎 1430。

首次运行自动生成独立随机数据库密码、账号签名密钥、管理员凭据。真实账号保存在 Docker volume `orbit-demo-postgres`；学习资料、路径和进度保存在 `.orbit-server/accounts/`，按账户隔离。浏览器使用 HttpOnly 会话 Cookie，刷新和再次登录可以继续原来的学习。

## 真实 AI 配置

在 `.env.local` 填入 `ORBIT_AI_BASE_URL`、`ORBIT_AI_API_KEY`、`ORBIT_AI_MODEL`，可选 `ORBIT_EMBEDDING_MODEL` 和 `ORBIT_BRAVE_API_KEY`。这是服务器配置，不进入前端 bundle。修改后重启学习服务。没有配置或上游失败时直接报错，绝不静默切换演示课程。资料导入不依赖 embedding：若上游没有 embedding 模型，资料仍会以关键词/FTS 检索参与路线生成，只是不建立向量索引。没有搜索密钥时可以直接输入主题或导入自己的资料。

初始数据库为空，不自动创建路线或完成节点。首次体验依次选择明暗背景、注册/登录、添加第一条路径。管理面板可查看真实用户与服务状态，以及调整 free/pro 套餐。浏览器学习引擎的服务器 AI 与官方云 AI 网关的配额是独立配置。

## Docker 打包与部署

`docker compose --env-file .env.local -f deploy/compose.yml build`

本地同时保留 Vite 调试服务时，可换端口冷启动验证：

```powershell
$env:ORBIT_HTTP_BIND='127.0.0.1'
$env:ORBIT_HTTP_PORT='11420'
$env:ORBIT_PUBLIC_ORIGIN='http://127.0.0.1:11420'
docker compose -p orbit-compose-e2e --env-file .env.local -f deploy/compose.yml up -d --wait --wait-timeout 240
```

`--wait` 会等 postgres、cloud、web 三个 healthcheck 全部 healthy；`http://127.0.0.1:11420/healthz` 应返回 `aiConfigured: true`。停止本机 Vite 后，也可直接用默认端口启动正式编排。

导出现场镜像：

```powershell
docker save orbit-demo:local orbit-demo-cloud:local postgres:16-alpine -o orbit-demo-images.tar
```

现场必须保留 compose 文件、独立密钥配置和数据卷备份。更完整的服务器步骤见 `deploy/README.md`，可使用 `deploy/deploy.sh` 或 `deploy/deploy.ps1` 一键启动。

部署后可运行验收脚本：

```powershell
pwsh -File scripts\smoke-production.ps1 -BaseUrl https://orbit.example.com
pwsh -File scripts\smoke-production.ps1 -BaseUrl https://orbit.example.com -CreateRoute -TimeoutSeconds 600
```

Compose 默认只把 Web 服务监听在本机 `127.0.0.1:1420`，cloud 以 `production` 模式运行。部署到远程服务器时设置：

- `ORBIT_HTTP_BIND=0.0.0.0`：允许外部访问；生产环境建议只让反向代理访问。
- `ORBIT_HTTP_PORT=1420`：调整宿主机端口。
- `ORBIT_PUBLIC_ORIGIN=https://你的域名`：必须与实际访问协议/域名一致；`https://` 会让会话 Cookie 带上 Secure。
- `VERRUCKT_ENV=production`：默认值；会强制要求 `DATABASE_URL` 和 `VERRUCKT_ACCESS_TOKEN_SECRET`，不要退回 development。

数据库和管理代理不会暴露到公网。生产上线时请在 Web 服务前配置 HTTPS 反向代理和防火墙。镜像构建需要能访问 Docker Hub；若默认镜像源不稳定，可使用可用的 Docker Hub 代理先拉取基础镜像并重新打回 `node:24-bookworm-slim`、`rust:1-bookworm`、`rust:1.97-bookworm`、`postgres:16-alpine`，然后再执行 compose build。也可先在能联网的机器构建后用 `docker save`/`docker load` 导入。

## 桌面安装包

```powershell
npm run tauri build --bundles nsis
```

生成 NSIS 和 MSI：

```powershell
npm run tauri build --bundles nsis,msi
```

生成商店用 MSIX：

```powershell
pwsh -File scripts\build-msix.ps1
```

输出：

- `src-tauri/target/release/bundle/nsis/Orbit_0.1.0-rc.1_x64-setup.exe`
- `src-tauri/target/release/bundle/msi/Orbit_0.1.0_x64_en-US.msi`
- `src-tauri/target/release/bundle/msix/Orbit_0.1.0.0_x64.msix`

MSIX 已通过 `makeappx unpack` 校验；正式上架前需要真实代码签名和 Partner Center Publisher，详见 `docs/STORE.md`。

## 发布打包

```powershell
pwsh -File scripts\package-release.ps1
```

会生成离线服务器包 `orbit-deploy-<version>.tar.gz` 和 Windows 安装包 `orbit-windows-<version>.zip`，以及校验和文件。该脚本不会包含 `.env.local` 或任何真实密钥。

## 停止

通过 `.orbit-server/cloud.pid` 和 `learning.pid` 确认对应进程后停止；数据库用 `docker stop orbit-demo-postgres`。不要删除数据卷或 `.orbit-server`，否则会丢失演示产生的真实数据。不要把 `.env.local`、管理凭据、数据目录打包进镜像或提交 Git。
