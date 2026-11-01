# Verruckt Cloud Deployment

Verruckt Cloud 的服务端实现及容器化部署配置随源码提供。

服务范围：

- Account
- Device
- Entitlement
- AI Gateway
- Sync
- Storage / Backup
- Quota

学习权威状态仍属于 Local Core：Course、Roadmap、LearningSession、Conversation、Concept / Mastery、Retrieval、Vector 与学习闭环不搬到 Cloud。

## 本地验证

仓库提供 Docker Compose 环境：

```powershell
.\scripts\e2e-cloud.ps1
```

该脚本会启动：

- PostgreSQL
- MinIO
- mock OpenAI-compatible upstream
- Verruckt Cloud

并执行真实 `cloud_live` integration tests。

本地 E2E 已覆盖账号、权益、AI Gateway、Sync、Quota、10-turn Socratic session 与 restart/resume。

> 本地 E2E 的 AI upstream 使用 OpenAI-compatible mock；真实生产模型需要配置真实 upstream 后另行验证。

## 生产部署

生产部署请参考：

- `cloud/README.md`
- `cloud/docker-compose.yml`
- `deploy/`
- `deploy/README.md`
- `deploy/init-env.sh`

生产环境至少需要：

- PostgreSQL
- S3-compatible object storage（MinIO / S3）
- HTTPS 反向代理
- AI upstream base URL + key + model
- 独立 secret 管理

不要把 token、password、API key 写入仓库。

## 客户端 endpoint 构建

客户端 endpoint 是 **build-time deployment configuration**：

```text
runtime override: VERRUCKT_CLOUD_URL
↓
persisted Settings value
↓
compile-time default: VERRUCKT_DEFAULT_CLOUD_URL
```

正式 Windows 客户端构建：

```powershell
.\scripts\build-with-cloud.ps1 `
  -CloudUrl "https://your-verruckt-cloud.example"
```

Release 构建要求 `https://`；localhost 只允许通过 `-AllowLocalHttp` 用于开发验证。

产物：

```text
src-tauri\target\release\bundle\nsis\学涯_0.1.0-rc.1_x64-setup.exe
```

## 状态说明

项目开发阶段未长期部署公共测试服务器。因此源码包不包含持续在线的公共 Verruckt Cloud 实例；评审方可自行部署 Cloud，并通过构建参数指定 Cloud Endpoint 重新生成客户端。

不同 Cloud Endpoint / account 代表不同 remote identity。`cloud_profile_owner` 等本地 profile ownership 保护继续保留，避免跨账号数据串用。
