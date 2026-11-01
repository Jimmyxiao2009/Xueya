# 学涯 / Orbit 比赛演示 Checklist

## Build

```powershell
.\scripts\build-with-cloud.ps1 -CloudUrl "https://your-verruckt-cloud.example"
```

产物：

```text
src-tauri\target\release\bundle\nsis\学涯_0.1.0-rc.1_x64-setup.exe
```

## Preflight

```powershell
$env:VERRUCKT_DEMO_EMAIL = "..."
$env:VERRUCKT_DEMO_PASSWORD = "..."
.\scripts\pre-demo-check.ps1 -CloudUrl "https://your-verruckt-cloud.example"
```

## Demo Smoke

```powershell
.\scripts\run-demo-smoke.ps1 -Mode Remote -CloudUrl "https://your-verruckt-cloud.example"
```

或本地 Compose 验证：

```powershell
.\scripts\run-demo-smoke.ps1 -Mode LocalCloud
```

## 出发前

- [ ] 最终 Windows installer 已复制到演示机。
- [ ] 备份 installer 已复制到 U 盘 / 第二台机器。
- [ ] Demo 账号有效：`official_ai` + `cloud_sync` + quota 充足。
- [ ] Primary Demo Course 已准备好。
- [ ] Backup Demo Course 已准备好。
- [ ] 手机热点可用。
- [ ] 确认最终安装包内嵌 Cloud endpoint，不依赖 `VERRUCKT_CLOUD_URL`。

## 上台前 30 分钟

- [ ] 运行：

```powershell
.\scripts\pre-demo-check.ps1 -CloudUrl "https://your-cloud.example" -Email $env:VERRUCKT_DEMO_EMAIL -Password $env:VERRUCKT_DEMO_PASSWORD
```

- [ ] 输出 `READY FOR DEMO`。
- [ ] 打开 Orbit / 学涯。
- [ ] 确认已登录正确 Demo 账号。
- [ ] 确认 Settings 显示学涯 AI 已连接。
- [ ] 确认 Primary Demo Course 可见。
- [ ] 确认 Learning 页面 composer 正常。

## 演示路径

1. 打开学涯，展示 Roadmap。
2. 进入 Primary Demo Course。
3. 开始 / 继续 LearningSession。
4. 展示 Agent 主动 opening。
5. 故意给出错误回答。
6. 展示 Agent Probe / Hint / Counterexample。
7. 修正回答。
8. 展示 Plan 推进。
9. 返回 Roadmap。
10. 重新进入同一节点，证明 Session Resume。

## 备用演示

- [ ] 如果时间允许，展示账号 / Cloud Sync。
- [ ] 如果 Cloud 异常，按 `COMPETITION_RECOVERY.md` 主动切换 BYOK / Offline，不要假装仍在使用 Cloud。
