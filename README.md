# Orbit

**把好奇心变成一条不断生长的学习路径。**

Orbit 是一个本地优先的桌面学习应用：把你手上的资料变成一条会随你作答而改变的
学习路线。

```
来源 → 综合 → 路线 → 学习 → 提问 → 诊断 → 调整 → 复习
```

它不是课程列表，也不是聊天框。路线会因为你答错而**插入一个加练节点**，并且
在首页告诉你为什么。

---

## 技术栈

| 层 | 选型 |
| --- | --- |
| 外壳 | Tauri 2（Rust） |
| 前端 | React 19 + TypeScript + Vite 8 |
| 样式 | Tailwind CSS 4（CSS-first `@theme`）+ 移植自原型的设计层 |
| 状态 | zustand |
| 数据 | SQLite（sqlx，本地优先，离线可用） |
| AI | BYOK，`ChatProvider` / `EmbeddingProvider` trait + `MockProvider` 兜底 |

---

## 跑起来

```bash
npm install
npm run tauri dev          # 开发（前端热更新 + Rust 热重载）
npm run tauri build        # 打包桌面应用
```

**不需要任何 API Key。** 没有配置 provider 时，`MockProvider` 会用确定性的
哈希向量提供检索与引用，路线、课程、题目照常生成。

首次启动自带两条演示路线：**射影几何**、**线性代数**，各有一个已完成节点。

---

## 验证

```bash
npm run typecheck          # tsc --noEmit
npm run lint               # eslint
npm run test               # vitest（前端 25 个测试）
npm run build              # tsc + vite build

cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                 # 42 个单元测试 + 10 个集成测试
```

---

## 目录

```
reference/            原型的原始单文件（设计的事实来源，不要删）
docs/
  ARCHITECTURE.md     分层、数据模型、学习闭环、AI 边界
  DESIGN_SYSTEM.md    token、动效常量、类名表、视觉回归清单
  BACKEND_API.md      每一条 Tauri IPC 命令、输入输出与错误码
  MIGRATION.md        原型 → 应用：保留了什么、改了什么、为什么
src/
  api.ts              唯一的 IPC 出口，全部 DTO 与类型化封装
  App.tsx             视图切换、手势绑定、课程卡展开动画的编排
  components/         layout · roadmap · lesson · learning · ui
  screens/            CreateRouteScreen · GeneratingScreen
  stores/             ui · route · job · lesson · learning · source
  lib/                swipeEngine · lessonTransition · roadPath · icons · format
  hooks/              useAmbientRoadmap · useControlFeedback · useVisualViewport …
  styles/             tokens · components · effects · motion · index
  test/               假 IPC 后端 + 端到端前端测试
src-tauri/
  src/ipc · services · domain · db/repo · ai · curriculum · ingest · knowledge · jobs
  migrations/         0001_init.sql · 0002_learning_loop.sql
  tests/learning_loop.rs
```

---

## 隐私

* 全部数据存在本机 SQLite，不联网也能用。
* API Key 只写进操作系统钥匙串，**永远不会回传给前端**；`provider_list`
  只返回 `hasApiKey: boolean`。
* 错误信息在序列化前会做密钥脱敏。
