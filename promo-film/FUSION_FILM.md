# 学涯 · 参赛融合版（操作录屏 + 中文讲解 + 动画 + 纯音乐）

## 当前交付

`final/competition_fusion_59s.mp4`。

用户约束：60 秒以内、20 MB 以内；取消 TTS，以纯音乐配实际操作录屏，同时保留中文讲解和适量动画。

## 剪辑时间轴

| 时间 | 画面 | 中文讲解重点 |
|---|---|---|
| 0–4 | 动态节点与连线，短开场 | 自学需要路线和理解反馈 |
| 4–12 | 实录输入“射影几何”，点击基础和深度 | 创建学习路线 |
| 12–16 | 实录路线与点击当前节点，镜头推进 | 看清当前进度与下一步 |
| 16–20 | 实录课程计划与点击开始学习 | 明确本课的小目标 |
| 20–32 | 实录输入错误回答、提交、导师追问 | 答错后继续引导思考 |
| 32–41 | 实录修正回答、确认概念，镜头聚焦引用 | 形成理解与资料依据 |
| 41–50 | 实录打开检查、选择答案、提交、显示反馈 | 发现误解、针对性复习 |
| 50–54 | “路线—对话—检查—复习”连线动画 | 学习过程的闭环 |
| 54–59 | 中文品牌落版与轨道动画 | 有路线、有引导、有反馈 |

## 音乐与素材

- 音轨只来自 `audio/fusion-score.wav`，没有旁白或 TTS。
- `audio/fusion-score.mjs` 程序合成 104 BPM 纯音乐：柔和键音、分解和弦、低音与轻打击乐，无外部采样。
- 在 Vite 开发预览中操作真实 React 页面，Playwright 录制原始 WebM。不是用截图拼接冒充操作。
- 导师与测验使用预设案例 IPC；画面标明“产品操作实录 · 预设案例”。不代表生产模型联网实测。
- 原始录屏：`recordings/fusion/take.webm`；浏览器解码用 MP4：`recordings/fusion/take.mp4`。
- 原始操作时间标记：`recordings/fusion/marks.json`。
- 运行时添加鼠标指示环，未修改产品源码。

## 重跑

根目录先运行 `npm run dev -- --host 127.0.0.1`。其余命令在 promo-film 下运行：

```powershell
node capture/fusion.mjs
ffmpeg -y -i recordings/fusion/take.webm -an -c:v libx264 -crf 18 -preset fast -g 30 -pix_fmt yuv420p -movflags +faststart recordings/fusion/take.mp4
node audio/fusion-score.mjs
node render/render.mjs --page scenes/fusion/film.html --from 0 --to 59 --fps 30 --out render/frames-fusion --workers 4 --port 8799
node render/finish-fusion.mjs
```

画面引擎按剪辑时间异步定位录屏帧；通用 renderer 已兼容 await render。最终合成只映射新的音乐文件，旧 narration.wav 不进入成片。

## 验证

编码：1080p / 30fps，H.264 两遍平均码率 2100 kbps + AAC 128 kbps，纯音乐响度归一化。

合成脚本要求 1770 帧、文件 <18,000,000 字节、时长 ≤60 秒，并完整解码。精确元数据见 `render/fusion-verification.json`；全片抽帧见 `render/fusion-contact.png`。

已检查录屏操作序列、对话放大位置、引用与反馈框选；字幕只解释实际展示的步骤。旧 `competition_zh_59s.mp4` 是含 TTS 的历史版本，旧 `final_candidate_v2.mp4` 是概念 PV，都不是当前推荐交付。

## 最终验收结果

- 59.000 秒，1920×1080，30fps，1770 帧。
- 16,689,255 字节（16.69 MB），低于 20 MB。
- AAC 48kHz，59.000 秒，仅来自 fusion-score.wav；无 TTS。
- 完整解码通过；抽查操作时点、中文字幕、引用与反馈框选。
- Vite 开发服务已按用户要求启动在 http://127.0.0.1:1420/，便于继续查看界面。
