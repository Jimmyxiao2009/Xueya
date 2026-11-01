# 学涯 · 参赛项目演示（中文）

## 交付约束

用户明确要求：60 秒以内、20 MB 以内；展示项目功能，不做抽象概念 PV；以中文为主。

成片：`final/competition_zh_59s.mp4`。59 秒，1920×1080，30fps。中文合成解说、烧录中文字幕；另附 `final/competition_zh.srt`。

## 内容

| 时间 | 讲解 | 画面 |
|---|---|---|
| 0–6 | 项目是什么、解决什么问题 | 学习路线 + 项目定位 |
| 6–13 | 输入学习主题与基础 | 新建主题、学习节奏真实界面 |
| 13–20 | 知识点路线与进度 | 射影几何路线 |
| 20–27 | 一课的学习步骤 | 本课目标、资料、解释、检查 |
| 27–37 | 回答有误时如何追问 | 导师提问 → 学习者答错 → 导师追问 |
| 37–46 | 修正理解与资料依据 | 学习者修正 → 导师确认与挑战 |
| 46–53 | 阶段检查与复习反馈 | 错误选项与具体反馈 |
| 53–59 | 项目实现与价值 | 本地优先；路线 + 对话 + 练习 |

## 素材依据

- 项目定位与本地优先架构：仓库 README.md。
- 核心演示路径：docs/COMPETITION_DEMO_CHECKLIST.md。
- 实际功能：CreateRouteScreen、LearningScreen、QuizCard、preview.tsx。
- 本轮重新操作真实 React 前端并截图，使用开发预览 fixtures 与既有的脚本化导师案例；并非生产 AI 请求录屏。画面标注“界面演示 · 预设案例”。
- 截图目录：shots/competition。旧版“反馈截图”没展示反馈，本轮已重新选择错误项并提交，捕获可见的阶段检查反馈。
- 中文声音：系统 Microsoft Huihui Desktop TTS；句子独立生成后同步，非真人配音。没有配乐掩盖解说。
- 未修改产品源文件；本轮只在 promo-film 编写制作文件，启动本地 Vite 用于截图。

## 重跑

在 promo-film 下执行（拍摄需根目录 Vite 运行于 1420）：

```powershell
node capture/competition.mjs
powershell -NoProfile -ExecutionPolicy Bypass -File audio/competition/narrate.ps1
node audio/competition/mix.mjs
node render/render.mjs --page scenes/competition/film.html --from 0 --to 59 --fps 30 --out render/frames-competition-sync --workers 4 --port 8798
node render/finish-competition.mjs
```

编码采用 H.264 两遍平均码率 2100 kbps + AAC 128 kbps。脚本强制检查 <18,000,000 字节和 ≤60 秒，并完整解码验证。最终精确元数据：render/competition-verification.json。

旧 final_candidate_v2.mp4 为被用户否定的概念 PV，不作为当前参赛交付。

## 本轮验收

主视频 59.000 秒，1770 帧；音视频轨时长一致。已通过 FFmpeg 完整解码，并抽查产品创建、导师追问、确认与阶段反馈。字幕按各句实际合成音频时长同步；画面标题无覆盖。只展示预设案例，不宣称已完成生产 AI 或评审现场实测。
