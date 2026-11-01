# FINAL COMPETITION CONSISTENCY CHECK / 最终一致性检查

> 检查时间：2026-09-20。检查人：事实核验 Agent。
> **本轮范围**：申报书与 PPT 尚未产出，本轮检查对象是仓库内评委可能看到的现有材料（README、docs、store、安装产物元数据）。
> Agent A/B 的稿件产出后必须用 `COMPETITION_CLAIM_LEDGER.md` 重跑本检查，重点对照台账 §E 一票否决项。

---

## ✅ PASS（已一致，勿动）

1. **命名（代码与配置层）**：中文产品名"学涯"在 tauri.conf.json、窗口标题、Android strings 三处一致；英文 Orbit 在商店资产与安装包命名一致；Verruckt 仅出现在技术标识符，未污染产品面。
2. **产品定位（代码层）**：对话为课堂主界面、课文折叠、Quiz 收进「阶段检查」——`LearningScreen.tsx` 与定位完全一致，无"刷题 App"残留。
3. **架构边界（代码层）**：cloud/crates 全仓 grep 零引用本地学习服务；orbit-server 确为本地核心的 HTTP 传输层。
4. **AI 策略（代码层）**：默认 Offline、BYOK 硬报错、排除 mock://、ORBIT_REQUIRE_AI 强制点均核实存在。
5. **测试证据链**：`audit/competition-convergence/` 数字（71/78/74/63/19+13）与当前代码状态（HEAD 9dcc5ac + 收敛改动）一一对应，可追溯。
6. **自适应路线证据**：真实用户实测（new-user-live）+ 集成测试（learning_loop）+ 截图（completed-route.png）三重证据互洽。

---

## 🔴 必须修（评委可见的事实错误/自相矛盾）

1. **README.md L60 测试数字过时**："42 个单元测试 + 12 个集成测试"→ 改为当前口径（lib 78；集成按套件分列；或按 SOURCE_OF_TRUTH §6）。
2. **`docs/STORE.md` vs `docs/RELEASE_CHECKLIST.md` MSI 口径冲突**：前者称 MSI 已构建，后者 L37 标 SKIPPED。产物实际存在。统一为："MSI 产物存在，但当轮冻结流程跳过其验证"。
3. **`audit/competition-convergence/RUNTIME_BOUNDARY.md` 表名错误**：§1 与 §5 中的 `learning_mastery` 表不存在，实际为 `mastery_states`（0001_init.sql L151）。若仓库随材料提交，评委核验代码会发现不符。
4. **旧品牌截图**：`store/screenshots/01-route.png`、`02-lesson.png`、`03-tutor.png` 页头仍为旧"Orbit · 和你一起理解"品牌，与当前"学涯"UI 不一致。若 store/ 目录随材料展示，必须按新品牌重截。
5. **申报附件位空缺**：仓库内未找到任何申报书/PPT 附件（无 docx/pptx/pdf）。Agent A/B 的产出物到位前，"最终一致性"不成立——本文件必须重跑。

---

## 🟡 建议修（不致命，但影响评委观感）

1. **README 标题仅 "# Orbit"**：建议改为"# 学涯（Orbit）"，与窗口标题/产品名一致，避免评委第一眼只见英文名。
2. **`src-tauri/icons/` 与 `store/Orbit_*.png` 可能不同源**（08-21 vs 09-20）：人工比对，统一图标。
3. **MSIX 占位 Publisher `CN=Verruckt`**：若现场演示安装 MSIX，评委将看到开发代号。演示用 NSIS（已实测）或提前说明。
4. **`theme-integration/restored-lighthouse-dark.png` 右上角 fixture 品牌"lighthouse."**：进 PPT 前裁掉或换干净截图。
5. **`docs/ANDROID_DEVICE.md` 位置**：全文是移植指南，标题易被误读为"已支持设备"的证据。建议标题或开头加一行"状态：代码级适配完成，真机验证待完成"。
6. **已知体验问题备案**：首次路线生成约 229 秒、新建路线误报"已为你重排本课"（`new-user-live/REPORT.md`）。演讲若现场生成路线，需预置已生成好的账户规避等待。

---

## 附：Agent A / B 交稿前自查清单（对应台账 §E）

- [ ] 全文搜索"Verruckt"：仅可出现在技术实现语境且注明开发代号。
- [ ] 全文搜索"Android"：每处必须带"代码级适配/真机验证进行中"限定。
- [ ] 全文搜索测试数字：只能出现 SOURCE_OF_TRUTH §6 的数字。
- [ ] 全文搜索"Cloud/云"：不得描述为学习运行时；职责不超出七类（账号/设备/权益/AI 网关/同步/存储/配额）。
- [ ] 产品定位句：对话=课堂、Quiz=工具；不得出现"刷题/题库为核心"。
- [ ] 截图：无旧 Orbit 品牌；苏格拉底三连图与"依据"展开图用新截素材。
- [ ] AI 能力：不出现 ConceptState/MasteryEvidence "已落库"、不出现无证据性能数字。
