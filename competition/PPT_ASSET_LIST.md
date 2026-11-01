# PPT_ASSET_LIST · 学涯（Orbit）竞赛 PPT 素材清单

> 分类：**A 当前已有** / **B 需要从应用截图** / **C 需要重新绘制** / **D 需要 Agent C（证据与素材负责人）提供**
> 路径均相对仓库根目录 `H:\项目\CrossPlatform\学涯`。

---

## A · 当前已有（可直接使用）

| 编号 | 素材 | 路径 | 用途 |
| --- | --- | --- | --- |
| A-01 | 路线图截图（暗色，含继续卡与"已为你解锁下一站"toast） | `store/screenshots/01-route.png` | P7 备选、P13 小图 |
| A-02 | 课程页截图（LESSON 01，学习目标、本课待学、开始学习） | `store/screenshots/02-lesson.png` | P7 备选 |
| A-03 | 学习对话截图（学习计划 + 对话 + 理解检查卡 + "有疑问，随时打断我…"） | `store/screenshots/03-tutor.png` | **P5 中心页底衬** |
| A-04 | 浅色 Buoy 路线（含"为你插入 · 复习 对偶原理"紫色节点） | `audit-results/theme-integration/home-buoy-light.png` | **P7 主图（adaptation 实证）** |
| A-05 | 暗色路线（含"加练 · 概率基础与分数表示"） | `audit-results/new-user-live/completed-route.png` | P5 迷你路线 / P7 备选 |
| A-06 | 导入资料页（文件/URL/AI 帮找） | `audit-results/oobe-three-pages/desktop-materials.png` | **P8 主图** |
| A-07 | 主题四宫格原图：Buoy 明 / Buoy 暗 / Lighthouse 明 / Lighthouse 暗 | `audit-results/theme-integration/home-{buoy,lighthouse}-{light,dark}.png` | **P10 四宫格** |
| A-08 | 应用图标 150/300/71px | `store/Orbit_*.png` | P1 封面小标 |
| A-09 | 真实离线学习 trace（开场白原文、加练插入文案） | `audit/competition-convergence/native-learning-trace-2.log` | P5 对话文案事实依据 |
| A-10 | 真实模型生成的路线截图（live） | `audit-results/multi-agent/live-route.png` | 备选 |
| A-11 | OOBE 三页截图（主题/滑块/资料，桌面+移动） | `audit-results/oobe-three-pages/*.png` | 备选 |
| A-12 | 测试数字与验证矩阵 | `audit/competition-convergence/COMPETITION_CONVERGENCE_REPORT.md` §3 | P12 数据源 |
| A-13 | Golden Path 现场脚本 + 应急表 | `docs/COMPETITION_DEMO.md` §5/§6 | P13 内容源 |

## B · 需要从应用截图（赛前补截，均需真实环境）

| 编号 | 素材 | 截图要求 | 用途 |
| --- | --- | --- | --- |
| B-01 | **真实模型**苏格拉底对话（BYOK/Cloud）：含开场白 + 一次答错 + 反例追问 + 反例后学生纠正 | 竖屏窗口，对话区完整；必须真实模型，不得用 Mock 冒充 | **P5 升级：替换演绎气泡为真实截图**（若能拿到，P5 说服力翻倍） |
| B-02 | 对话中"依据 · …"展开引用片段（[C1] 真实 excerpt） | 展开态完整可见 | P8 右侧 |
| B-03 | 答错后路线插入「加练 · 无穷远点」的路线页（射影几何 golden path） | 首页 notice 条 + 紫色节点同框 | P5 迷你路线 / P7 |
| B-04 | 设置页：AI 来源三态（离线/BYOK/Cloud）+ 云账号区 | 桌面宽版 | P9 角标小图（可选） |
| B-05 | OOBE 主题输入页（"你想学什么"） | 桌面宽版 | P4 备选小图 |
| B-06 | 浏览器演示模式（orbit-server）首页 | 若现场演示走浏览器形态则需要 | P10 备选 |

> 截图规范：窗口 480px 宽（产品设备框标准）或 1280×720 桌面版；暗色 Lighthouse 优先以匹配 PPT 基调；B-01 若为浅色主题也可，PPT 会用圆角卡片包边。

## C · 需要重新绘制（PPT 内矢量图形）

| 编号 | 图形 | 规格 | 用途 |
| --- | --- | --- | --- |
| C-01 | 封面/收尾灯塔 + 雾 + 蜿蜒航路 + 节点（收尾页节点更多点亮） | 纯形状绘制，灯光 `#e6c084`，雾 rgba(148,168,178,.05–.10) | P1 / P14 |
| C-02 | "你问→它答" vs "主动教→观察→调整" 对比图 | 左灰直线箭头，右暖色蜿蜒虚线+节点 | P2 |
| C-03 | 散落资料 → 雾中断层 → 成型路径 转化图 | 五类资料图标（PDF/网页/笔记/视频/对话） | P3 |
| C-04 | 八环节学习闭环（来源→…→复习，首尾回连） | 节点圆点与产品路线图同款 | P4 |
| C-05 | **苏格拉底对话流（6 气泡 + 教学动作标签 + 加练系统条）** | 暗底气泡；Ask 灯光色 / Counterexample 橙 / 系统条紫；若 B-01 拿到则替换为截图+标注 | **P5 主视觉** |
| C-06 | Teaching Agent 决策流程图（四输入 → Agent → 13 动作药丸 → 诊断三分支） | 全片最重要图解；动作药丸 13 个 | P6 |
| C-07 | 导入 → 解析 → 分块 → 混合检索 → 教学上下文 管线 | 纵向五段 | P8 |
| C-08 | Local Core + Cloud 两层架构图（可断开的连接） | Local 60% 暖描边，Cloud 40% 冷细线 | P9 |
| C-09 | 测试矩阵横向条形图（6 类，绿色）+ 诚实缺口面板（雾灰） | 数据源 A-12 | P12 |
| C-10 | 现场 Demo 五步时间线（断网/重启两步高亮） | 横向箭头 | P13 |

## D · 需要 Agent C（证据 / 素材负责人）提供或确认

| 编号 | 事项 | 验收标准 | 截止 |
| --- | --- | --- | --- |
| D-01 | 确认 P12 最终测试计数（以前端 `npm run test`、桌面 `cargo test` 各套件、向量 workspace、云端 workspace 的最终日志为准） | 与 `audit/competition-convergence/` 最终日志逐项一致；若赛前再跑有变化，更新 P12 数字与总数 | 赛前 1 天 |
| D-02 | 提供 B-01 真实模型对话截图（BYOK 或 Cloud 官方 AI，**禁止 Mock 冒充**；Mock 截图必须带"离线回答"标注才可使用） | 对话含开场白 + 答错 + 反例/追问 + 加练插入系统条 | 赛前 2 天 |
| D-03 | Android 真机冒烟结论（`docs/ANDROID_DEVICE.md` §8 清单 9 项） | 有结果则 P10 改"真机已验证"；无结果维持"真机验证进行中"措辞 | 赛前 1 天 |
| D-04 | Windows 安装包 SHA-256 与版本号（`RELEASE_CHECKSUMS.txt`） | P12 或现场问答备查 | 赛前 1 天 |
| D-05 | 现场 Demo 环境确认：`VERRUCKT_COMPETITION_DEMO=1` 构建、BYOK Key 可用性、断网演练、投影分辨率 | 按 `docs/COMPETITION_DEMO.md` §5 脚本完整彩排一遍 | 赛前彩排 |
| D-06 | 若赛事要求团队/报名信息的固定格式页 | 提供赛事模板字段 | 制作前 |

## 诚实红线（素材使用约束）

1. B-01 若无法取得真实模型截图，P5 维持"演绎气泡 + 页脚注明演绎来源"方案，**不得**用 Mock 输出冒充真实 AI 质量。
2. 向量引擎只讲"专用存储布局显著降低查询开销（10k p50 ≈126ms → ≈2.2ms）"，**不讲**"快 50 倍"（`docs/VECTOR_INTEGRATION_REPORT.md` 已纠正该口径）。
3. Android 只说"构建链路与签名就绪、真机验证进行中"，不展示伪造的真机截图。
4. "不登录也能用"仅指桌面端本地模式；浏览器演示形态需要账号——路演口述时按现场演示形态选择准确措辞。
