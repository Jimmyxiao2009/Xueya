# Orbit 生产部署

该目录提供单机 Docker Compose 部署方案。默认组件：

- `postgres`：账号数据库，仅容器网络内访问。
- `cloud`：Rust 账号服务，生产模式运行，仅容器网络内访问。
- `web`：Rust 学习引擎 + 前端静态资源，默认只映射到宿主机 `127.0.0.1:1420`。

## 1. 准备配置

推荐直接生成 `.env` 并自动写入随机数据库密码、会话签名密钥和管理员 token：

```sh
sh deploy/init-env.sh
```

非交互式示例：

```sh
ORBIT_PUBLIC_ORIGIN=https://orbit.example.com \
ORBIT_AI_BASE_URL=https://your-ai-gateway.example.com/v1 \
ORBIT_AI_API_KEY=replace-with-your-api-key \
ORBIT_AI_MODEL=your-model-name \
sh deploy/init-env.sh
```

也可以手动复制并编辑模板：

```powershell
Copy-Item deploy\.env.example deploy\.env
```

编辑 `deploy/.env`，至少填写：

- `ORBIT_DB_PASSWORD`
- `VERRUCKT_ACCESS_TOKEN_SECRET`
- `ORBIT_ADMIN_TOKEN`
- `ORBIT_PUBLIC_ORIGIN`
- `ORBIT_AI_BASE_URL`
- `ORBIT_AI_API_KEY`
- `ORBIT_AI_MODEL`

不要在聊天、日志、Git 或镜像中保存 `deploy/.env`。

如果目标 Ubuntu/Debian 服务器尚未安装 Docker，可在服务器上执行：

```sh
sudo sh deploy/install-docker-ubuntu.sh
```

脚本会通过 Docker 官方 apt 仓库安装 Docker Engine 和 Compose 插件。离线服务器请提前准备 Docker 或使用系统自带包。

如果希望一条命令完成 Docker 安装、生成 `.env`、预检和部署：

```sh
sh deploy/bootstrap.sh
```

首次运行会交互式询问公网 HTTPS 域名、AI 网关、API key 和模型；已有 `.env` 时不会覆盖。


## 2. 构建与启动

本机已构建并验证：

```powershell
docker compose --env-file deploy\.env -f deploy\compose.yml build
```

启动：

```powershell
pwsh -File deploy\deploy.ps1
```

Linux 服务器：

```sh
cp deploy/.env.example deploy/.env
# 编辑 deploy/.env
sh deploy/preflight.sh   # 可选，deploy.sh 也会自动调用
sh deploy/deploy.sh
```

`deploy.sh` / `deploy.ps1` 会优先加载与脚本同目录的 `orbit-demo-images.tar`（发布包解压后即在该目录），然后以 `--pull never` 启动，确保离线包不会触发外网拉取；没有离线包或设置 `ORBIT_BUILD=1` 时会在服务器上构建。

`preflight.sh` 会检查 Docker、Compose、必填环境变量、端口和离线镜像包。两套部署脚本均已在本机用真实离线包完整执行通过，最终 postgres/cloud/web 全部 healthy。

## 3. 离线镜像包

在有网络的机器：

```powershell
docker save orbit-demo:local orbit-demo-cloud:local postgres:16-alpine -o orbit-demo-images.tar
```

把 `orbit-demo-images.tar`、`deploy/` 目录和填好的 `deploy/.env` 放到目标服务器，然后运行 `deploy/deploy.sh`。

离线包已包含 `orbit-demo:local`、`orbit-demo-cloud:local` 和 `postgres:16-alpine`，已在本机用 `docker load` + `docker compose up --pull never` 验证过无外网启动。

## 4. HTTPS 反向代理

推荐 Caddy：

```sh
caddy run --config deploy/Caddyfile.example
```

把 `Caddyfile.example` 中的域名替换为真实域名。Caddy 会自动申请和续期证书。

本机已用 Caddy 容器 + `tls internal` 模拟 HTTPS 反代：`/healthz` 正常，注册响应 Cookie 为 `HttpOnly; SameSite=Strict; Secure`，随后会话内 `cloud_get_me` 成功。生产环境只需把站点地址换成真实域名。

部署要求：

- `ORBIT_PUBLIC_ORIGIN=https://你的域名`
- `ORBIT_HTTP_BIND=127.0.0.1`
- 防火墙只开放 `80/443`，不要直接暴露 `1420`。
- 如果不用反向代理而直接暴露 `ORBIT_HTTP_BIND=0.0.0.0`，必须自行配置 TLS，否则会话 Cookie 不应被视为安全。

## 5. 运维

查看状态：

```sh
docker compose --env-file deploy/.env -f deploy/compose.yml ps
curl -fsS http://127.0.0.1:1420/healthz
```

web 的 healthcheck 会检查 `aiConfigured:true`。如果 AI 环境变量没填对，容器会处于 `unhealthy`，`deploy.sh` / `deploy.ps1` 的 `--wait` 会直接报错。

停止但保留数据：

```sh
docker compose --env-file deploy/.env -f deploy/compose.yml down
```

数据卷：

- `postgres`：账号、套餐、设备
- `learning`：`/data` 下的账号 SQLite、资料、学习路径与进度

备份：

```sh
sh deploy/backup.sh
```

备份写入 `deploy/backups/<UTC 时间戳>/`，包含：

- `postgres.dump`：PostgreSQL 自定义格式备份
- `learning.tgz`：`/data` 学习数据卷
- `metadata.txt`：卷名和时间戳

恢复（会覆盖当前数据）：

```sh
ORBIT_RESTORE_YES=1 sh deploy/restore.sh deploy/backups/<时间戳>
```

不要执行 `down -v`，除非确认要清空全部真实数据。

## 6. 从 Windows 一键推送到 Linux 服务器

本机先准备好：

- `deploy/.env`
- `.orbit-server/orbit-demo-images.tar`
- SSH 登录方式（密钥或密码）

然后执行：

```powershell
pwsh -File scripts\deploy-remote.ps1 `
  -Server 203.0.113.10 `
  -User root `
  -IdentityFile C:\keys\orbit_ed25519 `
  -RemoteDir /opt/orbit
```

脚本会：

1. 在服务器创建目录。
2. 上传 `compose.yml`、`deploy.sh`、`.env`、`orbit-demo-images.tar` 等文件。
3. 执行 `sh deploy.sh`，加载离线镜像并以 `--pull never` 启动。
4. 如 Docker 需要 root，可加 `-RemoteSudo`。

先用 `-DryRun` 预览步骤：

```powershell
pwsh -File scripts\deploy-remote.ps1 -Server 203.0.113.10 -DryRun
```

## 7. 上线前检查

部署后可在本机运行生产验收脚本：

```powershell
pwsh -File scripts\smoke-production.ps1 -BaseUrl https://orbit.example.com
```

验证健康、真实注册和会话。若要同时验证真实 AI 路线生成：

```powershell
pwsh -File scripts\smoke-production.ps1 -BaseUrl https://orbit.example.com -CreateRoute -TimeoutSeconds 600
```

- `ORBIT_AI_BASE_URL` / `ORBIT_AI_API_KEY` 有效，`/healthz` 返回 `aiConfigured: true`。
- `ORBIT_PUBLIC_ORIGIN` 与浏览器实际访问的 HTTPS 域名完全一致。
- 管理员 token 仅输入到管理后台，不写入前端或日志。
- 确认已完成一次真实注册、AI 路线生成和课程进入。
- Windows 桌面安装包需要单独签名；当前 `deploy/` 只负责服务端上线。

## 8. 发布包

在仓库根目录执行：

```powershell
pwsh -File scripts\package-release.ps1
```

生成：

- `.orbit-server/release/orbit-deploy-<version>.tar.gz`：离线服务器部署包，内含 `orbit-demo-images.tar`、Compose、预检/部署/备份/恢复脚本和文档。
- `.orbit-server/release/orbit-windows-<version>.zip`：NSIS/MSI/MSIX 安装包和商店合规文档。
- `.orbit-server/release/SHA256SUMS.txt`：校验和。

服务器侧解包后：

```sh
tar -xzf orbit-deploy-0.1.0-rc.1.tar.gz
cd orbit-deploy-0.1.0-rc.1
sh init-env.sh
# 或先复制 .env.example 并手动编辑
sh deploy.sh
```
