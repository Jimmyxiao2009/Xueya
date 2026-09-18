# 竞赛演示：可重置、可离线、可解释

本文档描述 `RELEASE HARDENING & COMPETITION GOLDEN PATH` 使用的演示路径。
它只用于开发/比赛构建，生产构建默认禁用。

## 1. 一键重置演示路线

开发构建中打开 `设置 → 竞赛演示 → 重置竞赛演示`，会连续请求两次确认，然后：

1. 删除上一条带 `competitionDemo` 标记的「射影几何」路线；
2. 复用内容寻址的两份本地 Markdown 资料：
   - `projective-plane.md`：射影平面与无穷远点
   - `homogeneous-coordinates.md`：齐次坐标
3. 真正执行解析、分块、确定性测试向量、索引与混合检索；
4. 通过现有 `CourseService::create_course` 生成路线，而不是写死查询答案；
5. 完成第一站，使第二站成为当前站。

普通学习者路线不会被删除；没有 `competitionDemo` 标记的同名路线也不会被删除。

后端命令为 `competition_demo_seed`，只在 `cfg!(debug_assertions)` 或
`VERRUCKT_COMPETITION_DEMO=1` 时可用。

## 2. 网络独立性

演示路线所需资料全部来自本地 fixture：

- 不需要 Verruckt Cloud；
- 不需要 BYOK；
- 不需要外网；
- 默认离线 mock provider 可以生成确定性课程计划与 tutor 回复。

如果 UI 显示“离线演示模式”，就绝不能同时声称正在使用 Cloud 官方 AI。

## 3. Golden Path 演示脚本

1. 启动应用，进入设置，重置竞赛演示；
2. 返回路线页，打开「射影几何」；
3. 确认第一站已完成、第二站为当前站；
4. 进入第二站，查看“本节由 N 份资料整理”；
5. 向 tutor 提一个与无穷远点/齐次坐标相关的问题；
6. 展开“依据 · …”，确认引用来自真实资料片段；
7. 完成一个客观题；答错后确认路线出现加练站或复习反馈；
8. 断网，重复一次提问；本地学习仍可用；
9. 重启应用，确认路线、进度、引用与 adaptation 仍在。

## 4. 自动化覆盖

`src-tauri/tests/competition_demo.rs` 会验证：

- 连续两次 reset 后只保留一条演示路线；
- 路线确实关联至少两份真实本地资料；
- metadata 同时包含 `competitionDemo` 与 `sourceIds`；
- 第一站为 `Completed`，第二站为 `Available`。

运行：

```powershell
Set-Location src-tauri
cargo test --test competition_demo -- --nocapture
```

## 5. 3–5 分钟现场路径（冻结版）

1. 启动 `Orbit`，等待首页出现「射影几何」路线。
2. 点顶部「射影几何，展开路线管理」→「重置竞赛演示」只在 dev/competition 构建出现；生产 NSIS 不显示。
3. 确认路线层第一站已完成、第二站「无穷远点」是当前站。
4. 进入第二站，等待「本课待学」与「课程资料」；点击「开始学习」。
5. 在输入框提问：`平行线在射影平面里相交于哪里？`，发送。
6. 回答出现后，点击消息下方的「依据 · 射影平面与无穷远点」，展开真实本地 fixture excerpt。
7. 选择客观题中的错误项（B/C），点击「提交答案」；路线随后出现 `加练 · 无穷远点`，这是 adaptation climax。
8. 返回路线，确认加练节点仍在；重启应用，确认路线、进度、引用、adaptation 仍持久化。
9. 如果现场断网：Cloud/AI 状态显示离线或等待连接，但本地路线、检索、引用、错题 adaptation 仍可完成；不要说“正在使用 Cloud 官方 AI”。
10. 如果 Cloud 恢复：回到设置，账号应自动从 `DegradedOffline` 回到 `Authenticated`，无需重新登录。

## 6. Emergency Demo Mode

| 现场故障 | 立即动作 | 结果 |
| --- | --- | --- |
| Cloud 不可用 | 切到离线演示；使用本地 fixture | 路线/检索/引用/quiz/adaptation 继续 |
| 官方 AI 不可用 | 切 BYOK；若 BYOK 也不可用，使用离线 mock | 本地学习继续，回答明确标注离线 |
| 路线状态脏/重复 | 设置 → 竞赛演示 → 重置竞赛演示（两次确认） | 重建唯一 demo 路线，普通路线保留 |
| 账号过期 | 不依赖账号，直接进入离线演示 | 本地学习不受影响 |
| Source index broken | 资料库 → 重新索引；必要时重置竞赛演示 | 重建 fixture 与索引 |
| 应用数据损坏 | 使用全新 profile 启动；演示 fixture 会重新播种 | 新 profile 可用 |
| 现场需要证明持久化 | 完成一次错题 adaptation 后重启应用 | 加练节点仍在 |

## 7. 冻结版自动化证据

- `src-tauri/tests/rc_golden_path.rs`：10 次真实本地 Golden Path，含 DB close/reopen。
- `src-tauri/tests/competition_demo.rs`：reset 幂等、真实 fixture、第一站 Completed、第二站 Available。
- `src-tauri/tests/learning_loop.rs`：错题 adaptation、DB reopen、幂等。
- `docs/RC_VALIDATION.md`：完整安装/离线/故障/宽度/a11y/安全证据。

