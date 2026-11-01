# COMPETITION ASSET INDEX / 竞赛素材索引

> 盘点时间：2026-09-20。全仓库共 141 个媒体文件（PNG + 1 ICO），**无任何录屏（mp4/webm/mov），无任何图片形式架构图**。
> 品牌分水岭：2026-09-20 约 11:50 起 UI 统一为「学涯」。**早于此时间的截图是旧"Orbit"品牌，禁止直接进 PPT。**

---

## 1. Logo / 图标

| 位置 | 内容 | 最新？ | 适合 PPT？ | 需重截？ |
| --- | --- | --- | --- | --- |
| `store/Orbit_300x300.png` | 商店主图标 300×300 | ✅ 09-20 05:24 脚本生成 | ✅ 首选 Logo | 否 |
| `store/Orbit_150x150.png`、`Orbit_71x71.png` | 小尺寸商店图标 | ✅ | 备用 | 否 |
| `src-tauri/icons/`（icon.ico、128x128.png 等） | 桌面应用图标 | ⚠️ 08-21，可能与商店图标不同源 | 需人工比对后决定 | 建议与 store 图标统一 |
| `src/assets/lighthouse.png` | 灯塔插画 480×1378（App 内主题插画） | ✅ 09-19 | ✅ 适合 PPT 扉页/装饰 | 否 |

## 2. 产品截图（按页面）

### 2.1 Roadmap / 首页（学习路线）

| 位置 | 内容 | 最新？ | 适合 PPT？ | 需重截？ |
| --- | --- | --- | --- | --- |
| `audit-results/new-user-live/completed-route.png` | 1280×720，Buoy Dark，**真实用户数据**：已掌握节点下方插入橙色"加练·概率基础与分数表示"，共 7 节点 | ✅ 09-20 12:49，学涯品牌 | ✅✅ **核心卖点首选** | 否 |
| `audit-results/desktop-adaptation/after-home.png` | 1280×720，学涯品牌首页 | ✅ 16:35 | ✅ | 否 |
| `audit-results/product-convergence/roadmap-buoy-light-desktop.png` | 1280×900，Buoy Light，含"为你插入·复习 对偶原理"节点（fixture 数据） | ✅ 11:12 | ✅ 浅色主题备选 | 否 |
| `store/screenshots/01-route.png` | 1440×900 | ❌ **旧 Orbit 品牌** | ❌ | **必须重截** |

### 2.2 Learning Conversation（学习对话）

| 位置 | 内容 | 最新？ | 适合 PPT？ | 需重截？ |
| --- | --- | --- | --- | --- |
| `audit-results/desktop-adaptation/after-learning.png` | 1280×720 Dark，对话+答题卡，学涯品牌 | ✅ 16:36 | ✅ 当前最佳对话图 | 否 |
| `audit-results/oobe-three-pages/live-learning.png` | 1280×720 Light，答对反馈+解析 | ✅ 16:12 | ✅ | 否 |
| `store/screenshots/03-tutor.png`、`.orbit-server/e2e/r22-browser-chat.png` | 提问界面 | ❌ 旧品牌 | ❌ | **必须重截** |
| **苏格拉底纠错三连图（答错→Agent 引导→修正）** | **缺失** | — | — | **必须新截**：按 `docs/COMPETITION_DEMO.md` §5 第 5–7 步操作并连拍三张 |

### 2.3 Source Import（资料导入）

| 位置 | 内容 | 最新？ | 适合 PPT？ | 需重截？ |
| --- | --- | --- | --- | --- |
| `audit-results/desktop-adaptation/after-oobe-materials.png` | OOBE 资料页（"为好奇心，带一点线索"） | ✅ 16:39 | ✅ | 否 |
| `audit-results/oobe-three-pages/desktop-materials.png` | 同页 Light 主题 1280×720 | ✅ 16:04 | ✅ | 否 |
| `audit-results/desktop-adaptation/after-sources.png` | 资料库 3 条真实网页资料"已索引，可被引用" | ✅ 16:42 | ✅ | 否 |
| **导师回答"依据·…"展开引用片段** | **缺失** | — | — | **必须新截**（COMPETITION_DEMO §5 第 6 步画面） |

### 2.4 Settings / Account / OOBE / 其他

| 位置 | 内容 | 最新？ | 适合 PPT？ | 需重截？ |
| --- | --- | --- | --- | --- |
| `audit-results/desktop-adaptation/after-settings.png` | 设置页 1280×720 | ✅ 16:38 | ✅ | 否 |
| `audit-results/desktop-adaptation/after-welcome.png` / `after-login.png` / `after-register.png` | 欢迎/登录/注册 | ✅ 16:38 | ✅ | 否 |
| `audit-results/desktop-adaptation/after-management.png` | 路线管理 | ✅ 16:35 | ✅ | 否 |
| `audit-results/desktop-adaptation/after-generation.png` | 路线生成中 | ✅ 16:40 | 备用 | 否 |
| `audit-results/oobe-three-pages/desktop-topic.png` / `desktop-sliders.png` / `desktop-materials.png` | OOBE 三页桌面版 1280×720 | ✅ 16:03–16:10 | ✅ | 否 |
| `audit-results/oobe-three-pages/mobile-topic.png` / `mobile-sliders.png` / `mobile-materials.png` | OOBE 三页移动版 390×844 | ✅ | ✅ 移动端证据 | 否 |
| `audit-results/desktop-adaptation/mobile-learning.png` / `mobile-generation.png` | 移动端学习/生成页 390×844 | ✅ | ✅ | 否 |
| `audit-results/multi-agent/live-route.png` | 多 Agent 逐节点生成状态（含失败节点） | ✅ 15:45 | ⚠️ 仅用于讲架构韧性，勿作主视觉 | 否 |

## 3. 主题变体（Lighthouse / Buoy × Light / Dark）

| 位置 | 内容 | 最新？ | 适合 PPT？ | 需重截？ |
| --- | --- | --- | --- | --- |
| `audit-results/theme-integration/restored-lighthouse-dark.png` | 1280×960 Lighthouse Dark 完整灯塔场景 | ✅ 11:53 | ✅ 主视觉候选；⚠️ 右上角有 fixture 品牌"lighthouse."字样，用前裁掉或说明 | 建议重截干净版 |
| `audit-results/theme-integration/restored-lighthouse-light.png` | Lighthouse Light 1280×960 | ✅ | ✅ | 同上 |
| `audit-results/theme-integration/restored-buoy-light.png` | Buoy Light | ✅ | ✅ | 否 |
| `audit-results/theme-integration/home-buoy-dark.png` | Buoy Dark 390×844 | ✅ | 移动端 | 否 |
| `audit-results/product-convergence/roadmap-lighthouse-dark-desktop.png`、`lesson-lighthouse-dark-desktop.png`、`oobe-lighthouse-dark-desktop.png` | Lighthouse Dark 三件套 1280×900 | ✅ 11:09–11:12 | ✅ | 否 |
| `audit-results/theme-integration/mixed-sea-buoy-dark.png`、`mixed-paper-lighthouse-light.png`、`mixed-oobe.png` | 背景×图标混搭证据 | ✅ | 仅作"主题正交"证据 | 否 |

## 4. 架构图

**缺失。** 仓库无任何 SVG/图片架构图，`docs/ARCHITECTURE.md` 等均为纯文本。
**必须新画**，建议两张：① 本地优先运行时边界（Local Core vs Cloud 七职责，按 `RUNTIME_BOUNDARY.md` §1）；② AI Provider 三模式（Offline/BYOK/Cloud + 不静默降级，按 `AI_INTEGRATION_MATRIX.md` §1）。

## 5. 测试/验证证据（内部用，不进 PPT 正文）

- `audit/competition-convergence/*.txt`：全部测试日志（数字见 SOURCE_OF_TRUTH §6）。
- `audit-results/*/REPORT.md`：六批实测报告（new-user-live、oobe-three-pages、theme-integration、desktop-adaptation、multi-agent、product-convergence）。
- `.orbit-server/e2e/`（step1~34、r22 系列）：数量多但**全部旧品牌**，754×480 的 smoke 系列分辨率低，仅作内部证据。

## 6. 演示录屏

**缺失。** 全仓库无 mp4/webm/mov。若申报材料需要视频，按 `docs/COMPETITION_DEMO.md` §5 的 3–5 分钟冻结脚本新录（注意：该脚本为桌面端路径；浏览器端可参照 `docs/LOCAL_DEMO.md`）。

---

## 7. 最强展示素材推荐（任务四结论）

1. **自适应加练（第一卖点）**：`audit-results/new-user-live/completed-route.png`——真实用户、真实错题、真实插入节点、学涯品牌，一图讲清"路线会适应你"。
2. **对话是课堂**：`audit-results/desktop-adaptation/after-learning.png`（Dark 宽屏）。
3. **资料变上下文**：`after-oobe-materials.png` + `after-sources.png` 组合（导入动作 + 索引完成态）。
4. **主题工程**：`restored-lighthouse-dark.png`（Lighthouse Dark）与 `roadmap-buoy-light-desktop.png`（Buoy Light）并排放，讲"双主题×明暗正交"。
5. **移动端**：`oobe-three-pages/mobile-*.png` 三连。
6. **Logo**：`store/Orbit_300x300.png`；扉页装饰 `src/assets/lighthouse.png`。

**三个必须补拍的缺口（按优先级）：**

1. 🔴 苏格拉底纠错三连图（答错 → Agent 引导 → 修正）——核心中的核心，目前只有文字证据（`new-user-live/REPORT.md` 第 5–6 步），无画面。
2. 🔴 导师回答"依据 · …"展开引用图——证明"资料变上下文"的最后一环。
3. 🟡 演示录屏（若申报要求视频）。
