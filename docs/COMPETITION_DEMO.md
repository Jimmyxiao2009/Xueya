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
