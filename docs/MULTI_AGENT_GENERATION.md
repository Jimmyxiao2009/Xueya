# 路线与节点 Agent

路线生成采用两阶段流程：大纲 Agent 负责整体目标、章节依赖和节点顺序；每个节点的内容 Agent 接收完整大纲、用户目标及本节点资料，独立生成讲解和配套题目。

大纲入库后即可返回路线页，不等待全部内容生成。每节点对应一个 `generate_node_content` 后台任务，状态记录在节点 metadata 的 `contentStatus` 中：`pending`、`generating`、`ready`、`failed`。

- 每节点有独立锁。后台任务和用户打开课程共用此锁，完成后复用缓存。
- 讲解与题目先通过结构校验，再在同一事务提交。单节点失败不影响其他节点，也不会填入伪装成 AI 的模板内容。
- 打开失败节点或点击“重试本节内容”只重试该节点。
- 服务重启后恢复 pending/generating 节点；ready 节点保留题目 ID 和学习记录。
- 进程共享并发额度，默认同时运行 4 个节点内容请求，可通过 `ORBIT_NODE_AGENT_CONCURRENCY=1..16` 设置。每节点仍有独立任务，超过额度的任务排队。
- 路线页仅在存在未完成节点时刷新状态。现有主页主题保留；首次创建使用三页引导，见 `OOBE_FLOW.md`。

## 本地模型配置

配置在被 Git 忽略的 `.env.local` 中，密钥不写入源码：

```dotenv
ORBIT_AI_BASE_URL=https://api.stepfun.com/step_plan/v1
ORBIT_AI_MODEL=step-3.7-flash
ORBIT_AI_PROTOCOL=chat_completions
ORBIT_AI_API_KEY=<本地密钥>
ORBIT_NODE_AGENT_CONCURRENCY=2
```

Step 3.7 Flash 使用 `reasoning_effort=low`。

最终聊天地址为 `https://api.stepfun.com/step_plan/v1/chat/completions`。模型 ID 来自此账户实际 `/models` 响应。

Cloud 仍负责注册和登录。此模型通过学习服务 BYOK 配置调用，不改变 Cloud 账户的官方 AI 权限。

此开发环境实测 Step Plan 有 429 限流，因而本地设为 2 个并发内容请求。429 重试采用退避等待，并尊重秒数形式的 Retry-After；超过 60 秒的冷却会返回失败，避免过早重试。生成状态轮询可容忍短暂网络错误，持续失败则显示恢复提示。

## 验证

`src-tauri/tests/node_agents.rs` 使用受控 HTTP 服务阻塞内容响应，验证大纲提前返回、节点并发上限、单节点失败隔离、空缓存保护、并发打开去重及成功缓存复用。它不使用真实密钥，不能替代真实模型质量验证。

运行：

```text
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features web-server --test node_agents
```
