# 最新：操作实录与动画融合版

**当前交付：`final/competition_fusion_59s.mp4`。** 59 秒，中文画面讲解，实际操作录屏、节点动画与纯音乐；没有 TTS。延续 60 秒 / 20 MB 约束。制作说明见 `FUSION_FILM.md`，验证数据见 `render/fusion-verification.json`。

以下版本均作历史记录。

---

# 当前交付已改为中文参赛项目演示

用户否定抽象概念 PV，要求 60 秒 / 20 MB。最新版本：`final/competition_zh_59s.mp4`，59 秒、中文解说字幕、真实产品界面演示。详细脚本与制作说明见 `COMPETITION_FILM.md`；技术验收见 `render/competition-verification.json`。

以下 54 秒版本仅保留为历史记录，不再是推荐交付。

---

# HANDOFF — ORBIT / 学涯 · 54 秒 Motion Pass

更新时间：2026-09-21。

## 本轮制作

- audio/score2.mjs 已完整改为 54 秒；配乐事件对应最新时间轴，score2.wav 已生成（48kHz stereo）。
- 修正 film2.js：产品擦入对话的镜头锚点对齐；25.0 秒进入对话，取消擦入后的短暂停留；S4/S5 扩张环保持屏幕尺度；38.2 秒地图开始展开，与 Understanding 退场重叠。
- 主帧目录 render/frames-v2-54：1620 帧，30fps。
- 无营销标语帧目录 render/frames-anim-notext-54：810 帧，15fps。
- 原 render/frames-v2、frames-anim、frames-anim-notext 是旧版帧，仅作历史参考。
- README_PROMO、NEW_NARRATIVE、STORYBOARD_V2、BEFORE_AFTER、SIGNAL_GREEN_CONTINUITY 已同步。
- 产品源码未修改。

## 合成入口

主帧已齐全，运行 `node render/finish-v2.mjs` 可重复输出：

- final/final_candidate_v2.mp4：54 秒主片，1920×1080，30fps，H.264 CRF17 + AAC。
- final/rough_cut_v2.mp4：54 秒粗剪，CRF18。
- final/narrative_animatic.mp4：54 秒 15fps 静音预演。
- final/orbit_v2_9x16.mp4：1080×1920 完整画面深色留边适配。
- final/orbit_v2_1x1.mp4：1080×1080 完整画面深色留边适配。
- render/contact_v2_final.png 与 motion_*_v2.png：接触表与运动条。

无营销标语版另行合成：

```powershell
ffmpeg -y -framerate 15 -i render/frames-anim-notext-54/f_%05d.jpg -c:v libx264 -crf 22 -preset medium -pix_fmt yuv420p -movflags +faststart -t 54 final/narrative_animatic_notext.mp4
```

## 取舍

- 9:16 / 1:1 保留整个横版构图，避免中心裁切截断问题、资料、UI 和品牌轨道；尚未做竖屏原生重排。
- `?noText=1` 隐藏开场问题、路线/扩展标语及片尾 tagline；产品原文、理解文案、仪器标签及品牌保留，并非完全无字。
- UI 来自剧本化 mock 的真实产品截图，依靠镜头运动呈现学习过程。
- 后续调整仍以缩短无功能停顿、提前下一动作优先，不增加通用转场，不回填到 58 秒。

## 验证状态

配乐时长已检查为 54.000 秒；PCM 峰值 −1.5 dB。关键帧检查覆盖纸→词→节点、路线→产品、产品→对话以及理解→地图。视频导出与最终验证结果见 QA_V2_54.md。
