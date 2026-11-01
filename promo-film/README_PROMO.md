# 最新：操作实录与动画融合版

**当前交付：`final/competition_fusion_59s.mp4`。** 59 秒，中文画面讲解，实际操作录屏、节点动画与纯音乐；没有 TTS。延续 60 秒 / 20 MB 约束。制作说明见 `FUSION_FILM.md`，验证数据见 `render/fusion-verification.json`。

以下版本均作历史记录。

---

# 当前参赛版

**优先看 `final/competition_zh_59s.mp4`**：59 秒中文项目演示，针对用户要求的 60 秒 / 20 MB 上限制作。中文解说 + 字幕，展示学习路线、导师追问与阶段反馈。详见 `COMPETITION_FILM.md`。

下面是历史概念 PV 工程说明。

---

# README_PROMO — 学涯 ORBIT 宣传片工程

一支 54 秒品牌宣传片（保留 60 秒 v1 对照）的完整、可重复渲染工程。
**产品仓库（`../`，即本目录之外的一切）在整个制作中为只读参考，未被修改。**

---

## 交付物

### v2 — 因果连续版（当前版本，先看这个）

| 文件 | 说明 |
|---|---|
| `final/final_candidate_v2.mp4` | **v2 主成片** · 1920×1080 · 30fps · 54.0s · H.264 CRF17 + AAC |
| `final/rough_cut_v2.mp4` | v2 粗剪（CRF18） |
| `final/narrative_animatic.mp4` | 叙事 animatic（15fps，无最终混音） |
| `final/narrative_animatic_notext.mp4` | **TEST B 无营销标语版**——隐藏开场问题、路线/扩展标语和片尾 tagline；保留产品原文、理解文案及品牌 |
| `final/orbit_v2_9x16.mp4` / `orbit_v2_1x1.mp4` | 竖屏 / 方形完整画面适配（深色留边） |
| `REEDIT_DIAGNOSIS.md` | v1 叙事问题诊断 |
| `NEW_NARRATIVE.md` | v2 因果链：一个问题（平行铁轨为何相交）贯穿全片 |
| `SIGNAL_GREEN_CONTINUITY.md` | 绿点 13 次形态变化的连续表 |
| `STORYBOARD_V2.md` | v2 分镜（每镜标注"因"） |
| `BEFORE_AFTER.md` | v1→v2 逐镜处置：保留/删除/重排/重做 |

最新 v2 使用完整画面适配 9:16 / 1:1，避免中央裁切截断问题、产品对话和轨道；不是竖屏原生重排。

v2 引擎：`scenes/v2/film2.js` + `scenes/v2/specimens2.js`（六份"同一问题"档案卡）。

```bash
# v2 全片渲染（确定性）
node render/render.mjs --page scenes/v2/film2.html --from 0 --to 54 --fps 30 --out render/frames-v2-54 --workers 4
# 无字测试版
node render/render.mjs --page scenes/v2/film2.html --qs "?noText=1" --from 0 --to 54 --fps 15 --out render/frames-anim-notext-54 --workers 4
# v2 配乐
node audio/score2.mjs
# 合成
ffmpeg -framerate 30 -i render/frames-v2-54/f_%05d.jpg -i audio/score2.wav \
  -c:v libx264 -crf 17 -preset slow -pix_fmt yuv420p -c:a aac -b:a 192k \
  -movflags +faststart -shortest final/final_candidate_v2.mp4
```

### v1 — 第一版（保留作对照）

| 文件 | 说明 |
|---|---|
| `final/final_candidate.mp4` | **主成片候选** · 1920×1080 · 30fps · 60.0s · H.264 CRF17 + AAC 192k |
| `final/rough_cut.mp4` | 第一版完整粗剪（CRF18） |
| `final/first_style_test.mp4` | 风格验证片段 · 0–10.5s（LOST → ARCHIVE 冻结前） |
| `final/orbit_9x16.mp4` | 竖屏裁切版（中央 607×1080，安全区构图） |
| `final/orbit_1x1.mp4` | 方形裁切版（中央 1080×1080） |
| `CREATIVE_DIRECTION.md` | 创意方向：产品理解 / 视觉语言 / motion grammar / 声音 |
| `STORYBOARD.md` | 逐段分镜（画面 + 声音） |
| `SHOT_LIST.md` | 32 镜 shot list，标注实拍/合成来源 |
| `shots/` | 13 张真实产品 UI 4K 母版截图（3840×2160） |
| `render/contact_sheet.png` | 全片 30 帧抽帧总览 |

## v2 合成与 QA

主帧渲染完成后运行 `node render/finish-v2.mjs`，生成主片、粗剪、15fps 静音预演、完整画面比例适配和 QA 接触表。主帧使用独立目录 `render/frames-v2-54`，不会混入旧 58 秒帧。

## v1 管线（全部可重跑）

```bash
# 1. 产品 UI 采集（需要产品 dev server: 仓库根目录 npm run dev）
node capture/capture.mjs            # → shots/*.png（Playwright 驱动真实前端 + 运行时 mock IPC）

# 2. 视觉引擎调试（关键帧静帧）
node render/preview.mjs "2.0,12.0,22.4,34.9,57.0"   # → render/preview/*.png

# 3. 全片逐帧渲染（确定性：同一 t 永远同一帧）
node render/render.mjs --from 0 --to 60 --out render/frames-full --workers 4

# 4. 程序化配乐（无采样、无版权风险）
node audio/score.mjs                # → audio/score.wav（48kHz 立体声）

# 5. 合成
ffmpeg -framerate 30 -i render/frames-full/f_%05d.jpg -i audio/score.wav \
  -c:v libx264 -crf 17 -preset slow -pix_fmt yuv420p -c:a aac -b:a 192k \
  -movflags +faststart -shortest final/final_candidate.mp4
```

架构：`scenes/film.html` + `scenes/film.js` 是确定性 Canvas 引擎（`window.__film.render(t)`），
`scenes/specimens.js` 是程序绘制的档案卡（月面图/潮汐表/鸟翼/几何证明/PDF/等高线/代码），
`render/render.mjs` 用 Playwright 逐帧驱动并落盘 JPEG 帧，ffmpeg 合成。

## 素材与字体来源

- **产品 UI 截图**：本仓库前端真实渲染（`preview.html?fixtures=1`，运行时 mock IPC 注入剧本化导师对话；未修改产品代码）。
- **灯塔品牌资产**：`assets/product/lighthouse.png`，复制自 `src/assets/lighthouse.png`（产品自有资产；v1 正片未使用，保留给官网/发布会物料）。
- **档案卡/图谱/雾/噪点**：全部程序绘制（`specimens.js` / `film.js` / `noise.js`），无外部图片、无 stock 素材。
- **音乐与音效**：`audio/score.mjs` 全程序合成（棕噪声海风、780Hz sonar ping、纸张噪声簇、三角波 pad），无采样。
- **字体**（系统字体，随 OS 授权）：
  - Palatino Linotype —— 拉丁衬线（旁白/标题）
  - 宋体 SimSun —— 中文展示（"学涯"字标、节点中文名）
  - Cascadia Mono / Consolas —— 仪器坐标/标签
  - Segoe UI / Microsoft YaHei —— 产品 UI 截图内自带

## 设计约束（下轮迭代请继续遵守）

1. 转场只允许：FOG / LIGHT / MAP ZOOM / CIRCULAR SCAN / CONNECTION / DECONSTRUCTION / OCCLUSION。
2. SIGNAL GREEN `#35d0a5` 只属于"测绘仪器层"；产品 UI 保持自己的暖琥珀灯光。
3. 遮 Logo 测试：任何镜头拿掉字标后仍应能被认出是 ORBIT。
4. 关键主体保持在中央 60% 安全区（9:16 / 1:1 裁切兼容）。
5. Verruckt 代号不出现在任何画面、文案、元数据中。

## 已知取舍（v1）

- 对话内容为剧本化 mock（结构真实、文案为编剧），拍摄时产品 AI 的真实流式输出未接入。
- S4 的镜头运动是对 4K 静帧的后期推轨，非实时录屏；需要真录屏时可再接 Playwright video。
- 9:16 / 1:1 目前是中央裁切；竖屏原生重排版是下一轮工作。
