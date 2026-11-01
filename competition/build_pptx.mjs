// 学涯（Orbit）竞赛路演 PPT 生成脚本
// 视觉：Lighthouse 暗雾基调（与产品设计系统同源）
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const pptxgen = require('C:/Users/Ouyang Quan/AppData/Roaming/npm/node_modules/pptxgenjs');

const p = new pptxgen();
p.layout = 'LAYOUT_WIDE';
p.author = '学涯 Orbit';
p.title = '学涯 Orbit · 竞赛路演';

// ---------- 设计常量（与产品 token 对齐） ----------
const W = 13.333, H = 7.5, M = 0.55;
const BG = '0D1114';      // 深海雾底
const PANEL = '131A1F';   // 面板
const PANEL2 = '182229';  // 气泡/次面板
const TEXT = 'E8EAE7';    // 主文本
const MUTED = 'A9B2B0';   // 次文本
const DIM = '7E8A90';     // 脚注
const LIGHT = 'E6C084';   // 灯塔灯光（主强调）
const LIGHT_SOFT = 'F4E6C4';
const ACCENT = 'D9A866';  // 暖橙（当前/反例）
const SUCCESS = 'B9D0A8'; // 掌握绿
const PURPLE = '8D97A3';  // 复习紫
const LINE = '2A343C';    // 暗色发丝线
const FOGC = '94A5B2';    // 雾
const F = '微软雅黑';
const NUM = 'Georgia';

const A = 'H:/项目/CrossPlatform/学涯';
const IMG = {
  tutor: `${A}/store/screenshots/03-tutor.png`,
  routeAdapt: `${A}/audit-results/theme-integration/home-buoy-light.png`,
  materials: `${A}/audit-results/oobe-three-pages/desktop-materials.png`,
  thBuoyL: `${A}/audit-results/theme-integration/home-buoy-light.png`,
  thLightL: `${A}/audit-results/theme-integration/home-lighthouse-light.png`,
  thBuoyD: `${A}/audit-results/theme-integration/home-buoy-dark.png`,
  thLightD: `${A}/audit-results/theme-integration/home-lighthouse-dark.png`,
};

const sh = p.shapes;
const mkShadow = () => ({ type: 'outer', color: '000000', blur: 10, offset: 3, angle: 90, opacity: 0.35 });

function base(kicker, title, pageNo) {
  const s = p.addSlide();
  s.background = { color: BG };
  if (kicker) s.addText(kicker, { x: M, y: 0.4, w: 8, h: 0.3, fontFace: F, fontSize: 11.5, bold: true, color: LIGHT, charSpacing: 3, margin: 0 });
  if (title) s.addText(title, { x: M, y: 0.7, w: 11.2, h: 0.62, fontFace: F, fontSize: 27, bold: true, color: TEXT, margin: 0 });
  if (pageNo) {
    // 签名 motif：迷你航路（虚线 + 节点）+ 页码
    s.addShape(sh.LINE, { x: 11.62, y: 0.58, w: 0.5, h: 0, line: { color: DIM, width: 1, dashType: 'dash' } });
    s.addShape(sh.LINE, { x: 12.22, y: 0.58, w: 0.5, h: 0, line: { color: DIM, width: 1, dashType: 'dash' } });
    s.addShape(sh.OVAL, { x: 11.5, y: 0.53, w: 0.1, h: 0.1, fill: { color: SUCCESS } });
    s.addShape(sh.OVAL, { x: 12.12, y: 0.53, w: 0.1, h: 0.1, fill: { color: ACCENT } });
    s.addShape(sh.OVAL, { x: 12.72, y: 0.53, w: 0.1, h: 0.1, line: { color: DIM, width: 1 } });
    s.addText(`${String(pageNo).padStart(2, '0')} / 14`, { x: 11.4, y: 0.72, w: 1.5, h: 0.25, fontFace: NUM, fontSize: 10.5, color: DIM, align: 'right', margin: 0 });
  }
  return s;
}

function lighthouse(s, ox, oy, scale, opts = {}) {
  // 极简灯塔：光晕 + 光束 + 塔身 + 灯室 + 海雾
  s.addShape(sh.OVAL, { x: ox - 1.15 * scale, y: oy - 0.75 * scale, w: 2.3 * scale, h: 1.5 * scale, fill: { color: LIGHT, transparency: 90 } });
  s.addShape(sh.RECTANGLE, { x: ox - 3.6 * scale, y: oy - 0.32 * scale, w: 3.35 * scale, h: 0.42 * scale, rotate: -7, fill: { color: LIGHT, transparency: 82 } });
  s.addShape(sh.TRAPEZOID, { x: ox - 0.42 * scale, y: oy + 0.32 * scale, w: 0.84 * scale, h: 2.35 * scale, fill: { color: PANEL2 }, line: { color: '3A444E', width: 1 } });
  s.addShape(sh.RECTANGLE, { x: ox - 0.3 * scale, y: oy, w: 0.6 * scale, h: 0.32 * scale, fill: { color: LIGHT } });
  s.addShape(sh.OVAL, { x: ox - 0.25 * scale, y: oy - 0.16 * scale, w: 0.5 * scale, h: 0.22 * scale, fill: { color: LIGHT, transparency: 25 } });
  const fogN = opts.fog ?? 3;
  for (let i = 0; i < fogN; i++) {
    s.addShape(sh.ROUNDED_RECTANGLE, { x: ox - 2.6 * scale + i * 0.4 * scale, y: oy + (2.75 + i * 0.38) * scale, w: 5.2 * scale, h: 0.26 * scale, rectRadius: 0.13 * scale, fill: { color: FOGC, transparency: 90 - i * 2 } });
  }
}

function nodeDot(s, x, y, kind) {
  // kind: lit(绿) / current(橙+光晕) / unlit(描边)
  if (kind === 'current') s.addShape(sh.OVAL, { x: x - 0.08, y: y - 0.08, w: 0.3, h: 0.3, fill: { color: ACCENT, transparency: 75 } });
  if (kind === 'unlit') s.addShape(sh.OVAL, { x, y, w: 0.14, h: 0.14, line: { color: DIM, width: 1.2 } });
  else s.addShape(sh.OVAL, { x, y, w: 0.14, h: 0.14, fill: { color: kind === 'lit' ? SUCCESS : ACCENT } });
}

// ============================================================ P1 封面
(() => {
  const s = p.addSlide();
  s.background = { color: BG };
  lighthouse(s, 10.55, 2.55, 1.0);
  // 航路：4 节点蜿蜒
  const pts = [[0.95, 6.15], [2.15, 5.8], [3.35, 6.1], [4.55, 5.7]];
  for (let i = 0; i < 3; i++) s.addShape(sh.LINE, { x: pts[i][0] + 0.12, y: Math.min(pts[i][1], pts[i + 1][1]) + 0.06, w: pts[i + 1][0] - pts[i][0] - 0.1, h: Math.abs(pts[i + 1][1] - pts[i][1]), flipV: pts[i + 1][1] < pts[i][1], line: { color: DIM, width: 1.2, dashType: 'dash' } });
  nodeDot(s, pts[0][0], pts[0][1], 'lit');
  nodeDot(s, pts[1][0], pts[1][1], 'lit');
  nodeDot(s, pts[2][0], pts[2][1], 'current');
  nodeDot(s, pts[3][0], pts[3][1], 'unlit');
  s.addText('学涯 ORBIT · 竞赛路演 · 2026', { x: 0.9, y: 1.15, w: 7.5, h: 0.35, fontFace: F, fontSize: 13, bold: true, color: LIGHT, charSpacing: 4, margin: 0 });
  s.addText([
    { text: '让 AI 从「回答者」', options: { breakLine: true } },
    { text: '变成「陪你思考的老师」', options: {} },
  ], { x: 0.88, y: 1.7, w: 7.6, h: 2.1, fontFace: F, fontSize: 38, bold: true, color: TEXT, margin: 0, lineSpacingMultiple: 1.15 });
  s.addText('学涯是一个以苏格拉底式对话为核心的本地优先学习 Agent：\n它不只给答案 —— 它主动提问、观察你的理解，并因此改变你的学习路线。', { x: 0.9, y: 4.05, w: 7.2, h: 0.95, fontFace: F, fontSize: 14.5, color: MUTED, margin: 0, lineSpacingMultiple: 1.3 });
  s.addText('苏格拉底式对话　·　会生长的学习路线　·　本地优先', { x: 0.9, y: 6.75, w: 8, h: 0.35, fontFace: F, fontSize: 13, color: LIGHT_SOFT, margin: 0 });
  s.addText('开发代号 Verruckt', { x: 10.9, y: 6.9, w: 2, h: 0.3, fontFace: F, fontSize: 11, color: DIM, align: 'right', margin: 0 });
  s.addNotes('各位评委好。这是学涯。今天我只想讲一件事：AI 已经很会回答问题了，但回答问题不等于会教人。学涯想做的，是让 AI 开始学会怎样教人。');
})();

// ============================================================ P2 一个问题
(() => {
  const s = base('02 · 一个问题', 'AI 会回答，但它会教吗？', 2);
  // 左：普通 AI
  s.addShape(sh.ROUNDED_RECTANGLE, { x: 0.55, y: 1.75, w: 5.95, h: 4.35, rectRadius: 0.09, fill: { color: PANEL }, line: { color: LINE, width: 1 } });
  s.addText('普通 AI · 问答工具', { x: 0.55, y: 2.05, w: 5.95, h: 0.4, fontFace: F, fontSize: 15, bold: true, color: MUTED, align: 'center', margin: 0 });
  const lc = ['你问', '它答', '结束'];
  lc.forEach((t, i) => {
    s.addShape(sh.ROUNDED_RECTANGLE, { x: 1.0 + i * 1.83, y: 2.95, w: 1.45, h: 0.66, rectRadius: 0.08, fill: { color: PANEL2 }, line: { color: LINE, width: 1 } });
    s.addText(t, { x: 1.0 + i * 1.83, y: 2.95, w: 1.45, h: 0.66, fontFace: F, fontSize: 14, color: MUTED, align: 'center', valign: 'middle', margin: 0 });
    if (i < 2) s.addText('→', { x: 2.47 + i * 1.83, y: 3.0, w: 0.33, h: 0.55, fontFace: F, fontSize: 20, color: DIM, align: 'center', margin: 0 });
  });
  s.addText('它不知道你懂没懂，也不在乎。', { x: 0.55, y: 4.35, w: 5.95, h: 0.4, fontFace: F, fontSize: 13, color: DIM, align: 'center', margin: 0 });
  // 右：学涯
  s.addShape(sh.ROUNDED_RECTANGLE, { x: 6.85, y: 1.75, w: 5.93, h: 4.35, rectRadius: 0.09, fill: { color: PANEL }, line: { color: LIGHT, width: 1.25 } });
  s.addText('学涯 · 教学 Agent', { x: 6.85, y: 2.05, w: 5.93, h: 0.4, fontFace: F, fontSize: 15, bold: true, color: LIGHT, align: 'center', margin: 0 });
  const rp = [[7.55, 2.95], [8.95, 3.35], [10.35, 2.95], [11.75, 3.35]];
  const rl = ['它先开口', '观察你的反应', '换一种方式', '陪你走到懂'];
  for (let i = 0; i < 3; i++) s.addShape(sh.LINE, { x: rp[i][0] + 0.14, y: Math.min(rp[i][1], rp[i + 1][1]) + 0.07, w: rp[i + 1][0] - rp[i][0] - 0.14, h: Math.abs(rp[i + 1][1] - rp[i][1]), flipV: rp[i + 1][1] < rp[i][1], line: { color: ACCENT, width: 1.2, dashType: 'dash' } });
  rp.forEach((pt, i) => {
    nodeDot(s, pt[0], pt[1], i === 0 ? 'current' : i === 3 ? 'lit' : 'unlit');
    s.addText(rl[i], { x: pt[0] - 0.6, y: pt[1] + 0.28, w: 1.4, h: 0.3, fontFace: F, fontSize: 11.5, color: TEXT, align: 'center', margin: 0 });
  });
  s.addText('你答错了，它给的不是答案，而是一个反例。', { x: 6.85, y: 4.35, w: 5.93, h: 0.4, fontFace: F, fontSize: 13, color: LIGHT_SOFT, align: 'center', margin: 0 });
  s.addText('回答是被动的；教学是主动的、会观察、会调整。', { x: 0.55, y: 6.5, w: 12.23, h: 0.5, fontFace: F, fontSize: 17, bold: true, color: TEXT, align: 'center', margin: 0 });
  s.addNotes('左边是我们熟悉的 AI：你问，它答，对话结束。它不知道你听懂没有。右边是学涯：你点开一节课，先说话的是它；你答错了，它不说「错误」，它给你一个反例，让你自己发现矛盾。这一页是学涯和一切 AI 问答工具的分界线。');
})();

// ============================================================ P3 痛点
(() => {
  const s = base('03 · 用户痛点', '资料很多，学习没有发生', 3);
  s.addText('学习者手里的输入 · 越攒越多', { x: 0.95, y: 1.5, w: 6, h: 0.3, fontFace: F, fontSize: 12, color: DIM, margin: 0 });
  const chips = ['PDF 文档', '网页收藏', '手写笔记', '视频资料', 'AI 对话记录'];
  const jit = [1.88, 2.02, 1.8, 1.96, 1.84];
  chips.forEach((t, i) => {
    s.addShape(sh.ROUNDED_RECTANGLE, { x: 0.95 + i * 2.37, y: jit[i], w: 1.95, h: 0.7, rectRadius: 0.08, fill: { color: PANEL }, line: { color: LINE, width: 1 } });
    s.addText(t, { x: 0.95 + i * 2.37, y: jit[i], w: 1.95, h: 0.7, fontFace: F, fontSize: 13, color: TEXT, align: 'center', valign: 'middle', margin: 0 });
  });
  // 雾带断层
  for (let i = 0; i < 3; i++) s.addShape(sh.ROUNDED_RECTANGLE, { x: 0.55, y: 3.2 + i * 0.36, w: 12.23, h: 0.28, rectRadius: 0.14, fill: { color: FOGC, transparency: 91 - i * 2 } });
  s.addShape(sh.LINE, { x: 6.67, y: 2.72, w: 0, h: 0.4, line: { color: DIM, width: 1.2, dashType: 'dash' } });
  s.addShape(sh.LINE, { x: 6.67, y: 4.4, w: 0, h: 0.4, line: { color: DIM, width: 1.2, dashType: 'dash' } });
  s.addText('断层 —— 没有人替你组织顺序、检验理解、安排复习', { x: 0.55, y: 3.52, w: 12.23, h: 0.4, fontFace: F, fontSize: 13.5, color: LIGHT_SOFT, align: 'center', margin: 0 });
  // 成型路径
  s.addText('学涯把它们变成 —— 适合「这个人」的学习过程', { x: 0.55, y: 4.88, w: 12.23, h: 0.4, fontFace: F, fontSize: 14.5, bold: true, color: TEXT, align: 'center', margin: 0 });
  s.addShape(sh.LINE, { x: 1.35, y: 5.62, w: 10.6, h: 0, line: { color: DIM, width: 1.2, dashType: 'dash' } });
  const steps = ['目标', '顺序', '提问', '反馈', '复习'];
  steps.forEach((t, i) => {
    const x = 1.6 + i * 2.55;
    nodeDot(s, x, 5.55, i < 2 ? 'lit' : i === 2 ? 'current' : 'unlit');
    s.addText(t, { x: x - 0.5, y: 5.85, w: 1.15, h: 0.3, fontFace: F, fontSize: 12.5, color: TEXT, align: 'center', margin: 0 });
  });
  s.addText('收藏 ≠ 学会　·　摘要 ≠ 理解', { x: 0.55, y: 6.55, w: 12.23, h: 0.45, fontFace: F, fontSize: 17, bold: true, color: LIGHT, align: 'center', margin: 0 });
  s.addNotes('我们都有这样的文件夹：几十个 PDF、上百个收藏、和 AI 的聊天记录。它们是资料，不是学习。资料到「学会」之间断了一截：没人替你决定先学什么、没人检查你真懂假懂、没人替你安排复习。学涯补的就是这一截。');
})();

// ============================================================ P4 学涯是什么
(() => {
  const s = base('04 · 学涯是什么', '一个会陪你走完的学习 Agent', 4);
  s.addText('把你手上的资料，变成一条会随你作答而改变的学习路线。', { x: 0.55, y: 1.55, w: 12.23, h: 0.5, fontFace: F, fontSize: 19, bold: true, color: LIGHT_SOFT, align: 'center', margin: 0 });
  // 闭环：虚线椭圆 + 8 节点
  s.addShape(sh.OVAL, { x: 2.97, y: 2.8, w: 7.4, h: 3.0, line: { color: DIM, width: 1.2, dashType: 'dash' } });
  const cx = 6.67, cy = 4.3, rx = 3.7, ry = 1.5;
  const labels = ['来源', '综合', '路线', '学习', '提问', '诊断', '调整', '复习'];
  const angs = [90, 45, 0, -45, -90, -135, 180, 135];
  labels.forEach((t, i) => {
    const a = (angs[i] * Math.PI) / 180;
    const x = cx + rx * Math.cos(a) - 0.575, y = cy - ry * Math.sin(a) - 0.25;
    const hot = t === '调整';
    s.addShape(sh.ROUNDED_RECTANGLE, { x, y, w: 1.15, h: 0.5, rectRadius: 0.25, fill: { color: hot ? PURPLE : PANEL2 }, line: { color: hot ? PURPLE : LINE, width: 1 } });
    s.addText(t, { x, y, w: 1.15, h: 0.5, fontFace: F, fontSize: 13, bold: hot, color: hot ? BG : TEXT, align: 'center', valign: 'middle', margin: 0 });
  });
  s.addText('全部环节运行在你的本机 ·「调整」一环让路线变活：答错会长出复习节点', { x: 0.55, y: 6.15, w: 12.23, h: 0.35, fontFace: F, fontSize: 12.5, color: MUTED, align: 'center', margin: 0 });
  s.addText('它不是：题库　·　出题器　·　摘要工具　·　套壳聊天　·　固定课程播放器', { x: 0.55, y: 6.6, w: 12.23, h: 0.4, fontFace: F, fontSize: 13, color: DIM, align: 'center', margin: 0, strike: true });
  s.addNotes('学涯做的事一句话：把你的资料变成一条学习路线，然后由一个 Agent 用对话陪你走完。这八个环节全部发生在你自己的机器上。注意「调整」这一环——你答错，路线本身会长出一个复习节点。它不是题库、不是摘要器、不是套壳——这些形态都不会「调整」。');
})();

// ============================================================ P5 核心 Demo 对话流
(() => {
  const s = base('05 · 核心 Demo', '它不说「错误」，它给你一个反例', 5);
  const bubbles = [
    { who: 'agent', pill: 'AGENT · 开场 · ASK', pillColor: LIGHT, y: 1.58, h: 0.86, text: '先不急着记定义——把「方向」本身看成一个点，它和普通的点有什么区别？' },
    { who: 'stu', y: 2.54, h: 0.54, text: '无穷远点……就是非常非常远的点吧。' },
    { who: 'agent', pill: 'AGENT · COUNTEREXAMPLE · 反例', pillColor: ACCENT, y: 3.18, h: 1.0, text: '如果只是「很远」，沿直线走足够久总能到达。可两条平行线永远等距——它们会在那里相交吗？' },
    { who: 'stu', y: 4.28, h: 0.54, text: '等等——按我的说法，平行线也能相交，这矛盾了。' },
    { who: 'agent', pill: 'AGENT · TRANSITION · 推进', pillColor: LIGHT, y: 4.92, h: 1.0, text: '矛盾正是关键：平行线确实相交——交在无穷远点。这正是射影平面要引入它的原因。' },
  ];
  bubbles.forEach(b => {
    const isA = b.who === 'agent';
    const x = isA ? 0.55 : 2.85, w = 6.5;
    s.addShape(sh.ROUNDED_RECTANGLE, { x, y: b.y, w, h: b.h, rectRadius: 0.09, fill: { color: isA ? PANEL2 : PANEL, transparency: isA ? 0 : 0 }, line: { color: isA ? LINE : ACCENT, width: 1 } });
    if (isA) {
      const pw = b.pill.length > 22 ? 2.9 : 2.0;
      s.addShape(sh.ROUNDED_RECTANGLE, { x: x + 0.16, y: b.y + 0.1, w: pw, h: 0.26, rectRadius: 0.13, fill: { color: b.pillColor } });
      s.addText(b.pill, { x: x + 0.16, y: b.y + 0.1, w: pw, h: 0.26, fontFace: F, fontSize: 10, bold: true, color: BG, align: 'center', valign: 'middle', margin: 0 });
    }
    s.addText(b.text, { x: x + 0.2, y: b.y + (isA ? 0.4 : 0), w: w - 0.4, h: b.h - (isA ? 0.44 : 0), fontFace: F, fontSize: 12.5, color: TEXT, valign: isA ? 'top' : 'middle', margin: 0, lineSpacingMultiple: 1.15 });
  });
  // 系统条
  s.addShape(sh.ROUNDED_RECTANGLE, { x: 0.55, y: 6.08, w: 8.3, h: 0.48, rectRadius: 0.09, fill: { color: PURPLE } });
  s.addText('路线更新 · 检测到误解 → 插入「加练 · 无穷远点」· 重启后仍在', { x: 0.55, y: 6.08, w: 8.3, h: 0.48, fontFace: F, fontSize: 12.5, bold: true, color: BG, align: 'center', valign: 'middle', margin: 0 });
  s.addText('对话按真实提示词与运行日志模式演绎（开场白句式取自真实记录）；现场演示为实时生成。教学动作标签共 13 种。', { x: 0.55, y: 6.72, w: 8.3, h: 0.55, fontFace: F, fontSize: 11, color: DIM, margin: 0, lineSpacingMultiple: 1.2 });
  // 右栏：迷你路线
  s.addShape(sh.ROUNDED_RECTANGLE, { x: 9.1, y: 1.58, w: 3.68, h: 5.55, rectRadius: 0.09, fill: { color: PANEL }, line: { color: LINE, width: 1 } });
  s.addText('路线 · 实时变化', { x: 9.35, y: 1.8, w: 3.2, h: 0.3, fontFace: F, fontSize: 12, bold: true, color: MUTED, margin: 0 });
  s.addShape(sh.LINE, { x: 9.68, y: 2.62, w: 0, h: 3.5, line: { color: DIM, width: 1.2, dashType: 'dash' } });
  const nodes = [
    { t: '射影平面基础', tag: '已掌握', kind: 'lit' },
    { t: '无穷远点', tag: '当前', kind: 'current' },
    { t: '加练 · 无穷远点', tag: '新插入', kind: 'review' },
    { t: '综合小测', tag: '待解锁', kind: 'unlit' },
  ];
  nodes.forEach((n, i) => {
    const y = 2.5 + i * 0.95;
    if (n.kind === 'review') s.addShape(sh.OVAL, { x: 9.61, y: y - 0.06, w: 0.26, h: 0.26, fill: { color: PURPLE } });
    else nodeDot(s, 9.61, y, n.kind === 'unlit' ? 'unlit' : n.kind);
    s.addText(n.t, { x: 9.98, y: y - 0.1, w: 2.7, h: 0.3, fontFace: F, fontSize: 12.5, bold: n.kind === 'review', color: n.kind === 'review' ? PURPLE : TEXT, margin: 0 });
    s.addText(n.tag, { x: 9.98, y: y + 0.18, w: 2.7, h: 0.26, fontFace: F, fontSize: 10.5, color: n.kind === 'review' ? PURPLE : DIM, margin: 0 });
  });
  s.addText('误解不只留在对话里 —— 它改变了课程结构本身', { x: 9.35, y: 6.55, w: 3.2, h: 0.5, fontFace: F, fontSize: 11, color: LIGHT_SOFT, margin: 0, lineSpacingMultiple: 1.2 });
  s.addNotes('这是学涯最重要的一页。注意三件事：第一，先说话的是 Agent，不是学生——点开一节课，它用一个诊断问题开场。第二，学生答错了，它全程没说「错误」，它给了一个反例，让学生自己撞上矛盾。第三——这次误解不只停留在对话里，路线图上真的长出了一个加练节点，重启应用它还在。对话改变了课程结构。Conversation 是课堂，路线是活的。');
})();

// ============================================================ P6 Agent 工作机制
(() => {
  const s = base('06 · 学习 Agent 如何工作', '每一轮，它都在做教学决策', 6);
  const inputs = [
    ['当前知识点', '学习目标与本课计划'],
    ['对话历史', '长会话自动摘要'],
    ['掌握度画像', '掌握度 · 置信度 · 错误次数'],
    ['检索证据', '[C1..Cn] · 按路线隔离'],
  ];
  inputs.forEach((it, i) => {
    const y = 1.9 + i * 1.18;
    s.addShape(sh.ROUNDED_RECTANGLE, { x: 0.55, y, w: 3.35, h: 1.0, rectRadius: 0.08, fill: { color: PANEL }, line: { color: LINE, width: 1 } });
    s.addText(it[0], { x: 0.8, y: y + 0.14, w: 2.9, h: 0.32, fontFace: F, fontSize: 13, bold: true, color: TEXT, margin: 0 });
    s.addText(it[1], { x: 0.8, y: y + 0.5, w: 2.9, h: 0.3, fontFace: F, fontSize: 11, color: MUTED, margin: 0 });
  });
  s.addShape(sh.RIGHT_ARROW, { x: 4.0, y: 4.0, w: 0.52, h: 0.36, fill: { color: ACCENT, transparency: 15 } });
  s.addShape(sh.OVAL, { x: 4.62, y: 3.42, w: 2.05, h: 1.5, fill: { color: PANEL2 }, line: { color: LIGHT, width: 1.5 } });
  s.addText([
    { text: 'Teaching', options: { breakLine: true } },
    { text: 'Agent', options: {} },
  ], { x: 4.62, y: 3.42, w: 2.05, h: 1.5, fontFace: F, fontSize: 16, bold: true, color: LIGHT, align: 'center', valign: 'middle', margin: 0 });
  s.addShape(sh.RIGHT_ARROW, { x: 6.77, y: 4.0, w: 0.52, h: 0.36, fill: { color: ACCENT, transparency: 15 } });
  // 右侧：动作药丸
  s.addText('下一步教学动作 · 13 选 1', { x: 7.4, y: 1.9, w: 5.4, h: 0.3, fontFace: F, fontSize: 12, color: MUTED, margin: 0 });
  const acts = ['提问', '探查', '提示', '举例', '反例', '讲解', '挑战', '反思', '小结', '推进', '练习', '测验', '概念完成'];
  acts.forEach((a, i) => {
    const col = i % 5, row = Math.floor(i / 5);
    const x = 7.4 + col * 1.1, y = 2.28 + row * 0.56;
    const hot = a === '反例';
    s.addShape(sh.ROUNDED_RECTANGLE, { x, y, w: 1.0, h: 0.44, rectRadius: 0.22, fill: { color: hot ? ACCENT : PANEL2 }, line: { color: hot ? ACCENT : LINE, width: 1 } });
    s.addText(a, { x, y, w: 1.0, h: 0.44, fontFace: F, fontSize: 11.5, bold: hot, color: hot ? BG : TEXT, align: 'center', valign: 'middle', margin: 0 });
  });
  s.addText('同时输出诊断：误解 / 答错 / 部分理解 / 低置信 / 反复失败 / 已掌握 / 不确定 —— 按置信度 × 严重度决策', { x: 7.4, y: 4.05, w: 5.4, h: 0.6, fontFace: F, fontSize: 11, color: MUTED, margin: 0, lineSpacingMultiple: 1.2 });
  const branches = [['＜0.55 · 不扰动路线', false], ['误解·反复失败 → 插入复习', true], ['低置信 → 安排复习', false]];
  branches.forEach((b, i) => {
    const x = 7.4 + i * 1.85;
    s.addShape(sh.ROUNDED_RECTANGLE, { x, y: 4.78, w: 1.72, h: 0.66, rectRadius: 0.08, fill: { color: b[1] ? PURPLE : PANEL }, line: { color: b[1] ? PURPLE : LINE, width: 1 } });
    s.addText(b[0], { x, y: 4.78, w: 1.72, h: 0.66, fontFace: F, fontSize: 10.5, bold: !!b[1], color: b[1] ? BG : TEXT, align: 'center', valign: 'middle', margin: 0, lineSpacingMultiple: 1.1 });
  });
  s.addShape(sh.LINE, { x: 0.55, y: 6.05, w: 12.23, h: 0, line: { color: LINE, width: 1 } });
  s.addText('先问后讲 · 先提示后答案 · 先探查后纠正', { x: 0.55, y: 6.25, w: 12.23, h: 0.42, fontFace: F, fontSize: 15, bold: true, color: LIGHT, align: 'center', margin: 0 });
  s.addText('教学动作：AgentActionKind 13 种 · 诊断置信度阈值 0.55 · 插入复习由 AdaptationService 幂等执行，重启后仍在', { x: 0.55, y: 6.82, w: 12.23, h: 0.3, fontFace: F, fontSize: 11, color: DIM, align: 'center', margin: 0 });
  s.addNotes('每一轮对话，Agent 拿到四样东西：当前知识点、历史对话、这个学生的掌握度和置信度、以及从他自己资料里检索到的证据。它输出的不是一段话，而是一个教学动作——提问、提示、举例、反例……共十三种。同时输出诊断：是误解还是部分理解？置信度够不够？低置信不扰动路线；确认误解，才插入复习。教学决策有阈值、可审计。');
})();

// ============================================================ P7 学习路线
(() => {
  const s = base('07 · 学习路线', '路线负责方向，Agent 负责陪走', 7);
  s.addImage({ path: IMG.routeAdapt, x: 0.7, y: 1.58, w: 2.47, h: 5.35, shadow: mkShadow() });
  s.addShape(sh.ROUNDED_RECTANGLE, { x: 0.7, y: 1.58, w: 2.47, h: 5.35, rectRadius: 0.09, line: { color: LINE, width: 1 }, fill: { color: BG, transparency: 100 } });
  s.addText('真实截图 · 紫色「为你插入 · 复习」节点由 Agent 插入', { x: 0.5, y: 7.02, w: 2.9, h: 0.4, fontFace: F, fontSize: 10.5, color: DIM, align: 'center', margin: 0, lineSpacingMultiple: 1.15 });
  const rows = [
    ['路线告诉你「要去哪里」', '章节、节点、依赖关系、预计时长 —— 学习目标始终显式可见，学生面对的不是一个无限聊天框。'],
    ['Agent 负责「怎么陪你走过去」', '开场提问、追问与提示、检查理解、必要时插入加练 —— 教学方式随你的每一次反应调整。'],
    ['答错会改变路线本身', '插入「加练 · …」节点，并在首页说明原因；掌握一站，点亮下一站。'],
  ];
  rows.forEach((r, i) => {
    const y = 1.95 + i * 1.68;
    s.addText(r[0], { x: 3.6, y, w: 9.1, h: 0.42, fontFace: F, fontSize: 16.5, bold: true, color: i === 2 ? LIGHT : TEXT, margin: 0 });
    s.addText(r[1], { x: 3.6, y: y + 0.5, w: 9.1, h: 0.75, fontFace: F, fontSize: 13, color: MUTED, margin: 0, lineSpacingMultiple: 1.25 });
    if (i < 2) s.addShape(sh.LINE, { x: 3.6, y: y + 1.42, w: 9.1, h: 0, line: { color: LINE, width: 1 } });
  });
  s.addShape(sh.LINE, { x: 3.0, y: 4.85, w: 0.5, h: 0.5, flipV: true, line: { color: PURPLE, width: 1.5, dashType: 'dash', endArrowType: 'triangle' } });
  s.addNotes('纯聊天框的问题是：学生不知道自己在哪、要去哪。学涯把学习目标和路径显式化——这条路线就是地图。而当你答错，地图上真的会长出新节点：这个紫色的「为你插入 · 复习」就是 Agent 插进来的，并在首页告诉你为什么。方向感和主动性，各司其职。');
})();

// ============================================================ P8 资料
(() => {
  const s = base('08 · 你的资料', '不是 RAG 聊天，是有据可依的教学', 8);
  s.addImage({ path: IMG.materials, x: 0.55, y: 1.58, w: 5.85, h: 3.29, shadow: mkShadow() });
  s.addShape(sh.ROUNDED_RECTANGLE, { x: 0.55, y: 1.58, w: 5.85, h: 3.29, rectRadius: 0.06, line: { color: LINE, width: 1 }, fill: { color: BG, transparency: 100 } });
  s.addText('导入页实拍 · 文件 / 网页链接 / AI 帮找', { x: 0.55, y: 4.95, w: 5.85, h: 0.3, fontFace: F, fontSize: 11, color: DIM, align: 'center', margin: 0 });
  const pipe = ['导入', '解析', '分块', '混合检索', '教学上下文'];
  pipe.forEach((t, i) => {
    const x = 0.55 + i * 1.17, last = i === 4;
    s.addText(t, { x, y: 5.5, w: 1.13, h: 0.52, fontFace: F, fontSize: 10.5, bold: last, color: last ? BG : TEXT, align: 'center', valign: 'middle', margin: 0, shape: sh.CHEVRON, fill: { color: last ? LIGHT : PANEL2 }, line: { color: last ? LIGHT : LINE, width: 1 } });
  });
  const cards = [
    ['source_grounded', SUCCESS, '回答来自你的资料：附 [C1] 引用，可展开查看原文片段'],
    ['model_knowledge', LIGHT, '来自模型自身知识 —— 明说，不冒充出自你的资料'],
    ['insufficient_source', MUTED, '资料里没有 —— 明说「没有」，不硬编答案'],
  ];
  cards.forEach((c, i) => {
    const y = 1.58 + i * 1.32;
    s.addShape(sh.ROUNDED_RECTANGLE, { x: 6.75, y, w: 6.03, h: 1.14, rectRadius: 0.08, fill: { color: PANEL }, line: { color: LINE, width: 1 } });
    s.addText(c[0], { x: 7.0, y: y + 0.14, w: 5.5, h: 0.3, fontFace: NUM, fontSize: 12, bold: true, color: c[1], margin: 0 });
    s.addText(c[2], { x: 7.0, y: y + 0.5, w: 5.5, h: 0.55, fontFace: F, fontSize: 12.5, color: TEXT, margin: 0, lineSpacingMultiple: 1.2 });
  });
  s.addText('引用标签由后端映射回真实资料片段，模型编造的标签直接丢弃 · 资料以「不可信内容」边界注入，防提示注入 · 检索按路线隔离', { x: 6.75, y: 5.6, w: 6.03, h: 0.9, fontFace: F, fontSize: 11, color: DIM, margin: 0, lineSpacingMultiple: 1.25 });
  s.addNotes('学涯不是把 PDF 丢给模型聊天。资料经过解析、分块、混合检索，成为 Agent 的教学证据，而且按路线隔离。最关键的三态标注：这句话来自你的资料、来自模型、还是资料里根本没有——三种情况三种标注，评委可以在对话里点开「依据」直接看到原文片段。诚实，是可验证的。');
})();

// ============================================================ P9 技术架构
(() => {
  const s = base('09 · 技术架构', '本地为核心，云端为增强', 9);
  s.addShape(sh.ROUNDED_RECTANGLE, { x: 0.55, y: 1.7, w: 6.55, h: 4.35, rectRadius: 0.09, fill: { color: PANEL }, line: { color: LIGHT, width: 1.25 } });
  s.addText('Local Core · Rust / Tauri', { x: 0.9, y: 1.95, w: 5.8, h: 0.4, fontFace: F, fontSize: 15, bold: true, color: LIGHT, margin: 0 });
  const li = ['学习会话与教学 Agent（对话是课堂）', '课程与路线 · 掌握度与适应', '混合检索与向量索引', 'SQLite 本地数据库 · 离线可用', 'AI Provider 接口：离线 / BYOK / Cloud'];
  s.addText(li.map((t, i) => ({ text: t, options: { bullet: { code: '25AA', indent: 12 }, breakLine: i < 4 } })), { x: 0.95, y: 2.5, w: 5.8, h: 3.3, fontFace: F, fontSize: 13, color: TEXT, paraSpaceAfter: 12, margin: 0 });
  s.addShape(sh.ROUNDED_RECTANGLE, { x: 8.5, y: 1.7, w: 4.28, h: 4.35, rectRadius: 0.09, fill: { color: PANEL }, line: { color: LINE, width: 1 } });
  s.addText('Cloud · 可选增强', { x: 8.8, y: 1.95, w: 3.7, h: 0.4, fontFace: F, fontSize: 15, bold: true, color: MUTED, margin: 0 });
  const ci = ['账号与设备 · 额度与权益', 'AI 网关（对话 / 向量）', '增量同步 · 冲突不静默覆盖', '对象存储（MinIO / S3）'];
  s.addText(ci.map((t, i) => ({ text: t, options: { bullet: { code: '25AA', indent: 12 }, breakLine: i < 3 } })), { x: 8.85, y: 2.5, w: 3.7, h: 3.3, fontFace: F, fontSize: 12.5, color: MUTED, paraSpaceAfter: 12, margin: 0 });
  s.addShape(sh.LEFT_RIGHT_ARROW, { x: 7.22, y: 3.5, w: 1.15, h: 0.5, line: { color: DIM, width: 1.5 } });
  s.addText('登录后同步\n可断开', { x: 7.06, y: 4.1, w: 1.45, h: 0.6, fontFace: F, fontSize: 10, color: DIM, align: 'center', margin: 0, lineSpacingMultiple: 1.15 });
  s.addText('AI 三模式：离线 · 自带 Key（BYOK）· Cloud 官方 AI ｜ 云故障自动降级并明确标注，学习不中断 ｜ 本地学习路径无需账号', { x: 0.55, y: 6.4, w: 12.23, h: 0.45, fontFace: F, fontSize: 13.5, bold: true, color: LIGHT_SOFT, align: 'center', margin: 0 });
  s.addNotes('架构只有一页，因为原则只有一条：本地优先。学习、数据、路线、检索全部在本机 Rust 核心里，登录和联网都是可选项。云端提供账号、同步和 AI 网关——它是同一个 Provider 接口的另一种实现，不是另一套系统。云挂了，学习继续，界面会诚实告诉你现在是离线。这一点等下现场断网演示。');
})();

// ============================================================ P10 多端与设计
(() => {
  const s = base('10 · 多端与设计', '一套核心，两种气质，多个端', 10);
  const shots = [[IMG.thBuoyL, '浮标 · 浅色'], [IMG.thLightL, '灯塔 · 浅色'], [IMG.thBuoyD, '浮标 · 深色'], [IMG.thLightD, '灯塔 · 深色']];
  shots.forEach((it, i) => {
    const x = 2.16 + i * 2.34;
    s.addImage({ path: it[0], x, y: 1.62, w: 1.99, h: 4.3, shadow: mkShadow() });
    s.addShape(sh.ROUNDED_RECTANGLE, { x, y: 1.62, w: 1.99, h: 4.3, rectRadius: 0.08, line: { color: LINE, width: 1 }, fill: { color: BG, transparency: 100 } });
    s.addText(it[1], { x, y: 6.02, w: 1.99, h: 0.3, fontFace: F, fontSize: 12, color: MUTED, align: 'center', margin: 0 });
  });
  s.addText('母题（浮标纸面 / 灯塔雾海）× 外观（明 / 暗）· 四种正交组合', { x: 0.55, y: 6.5, w: 12.23, h: 0.38, fontFace: F, fontSize: 14, bold: true, color: TEXT, align: 'center', margin: 0 });
  s.addText('Windows 安装包已验证 · Android 构建与签名就绪（真机验证进行中）· 动效承载意义：点亮 = 掌握，雾散 = 抵达', { x: 0.55, y: 6.95, w: 12.23, h: 0.32, fontFace: F, fontSize: 12, color: MUTED, align: 'center', margin: 0 });
  s.addNotes('设计不是皮肤，是叙事：灯塔母题里，学会一个节点就是点亮一座塔，进入新节点就是拨开一片雾。母题和明暗是两个独立维度，四种组合都可用。同一套 Rust 核心跑 Windows 和 Android——Windows 安装包已经过安装、启动、卸载验证；Android 构建链路就绪，真机验证还在进行，我们如实说明。');
})();

// ============================================================ P11 技术深度
(() => {
  const s = base('11 · 技术深度', '看得见的地方克制，看不见的地方较真', 11);
  const cards = [
    ['RUST CORE', 'Rust 本地核心', '学习、路线、检索、同步全部在 Rust；前端只渲染，不计算掌握度。', '78 项 lib 测试'],
    ['VECTOR ENGINE', '自研向量引擎', 'HNSW + 精确双索引；WAL 崩溃恢复，分代持久化。', '63 项测试'],
    ['AI PROVIDERS', '可替换 AI 架构', '离线 / BYOK / Cloud 三模式；失败显式报错，绝不静默退回 Mock。', '密钥只进 OS 钥匙串'],
    ['MULTI-AGENT', '多 Agent 课程生成', '大纲 Agent 先行返回路线；每节点独立内容 Agent，失败隔离。', '大纲 + N 个节点 Agent'],
    ['SYNC', '崩溃安全增量同步', 'outbox 与写入同事务；服务器确认才删除；MinIO 直传 + SHA-256 校验。', '冲突不静默覆盖'],
    ['DOMAIN MODEL', '对话是一等领域', 'LearningSession · AgentAction · Diagnosis · AdaptationEvent 显式建模。', '13 种动作 · 7 类诊断'],
  ];
  cards.forEach((c, i) => {
    const x = 0.55 + (i % 3) * 4.15, y = 1.62 + Math.floor(i / 3) * 2.52;
    s.addShape(sh.ROUNDED_RECTANGLE, { x, y, w: 3.93, h: 2.32, rectRadius: 0.09, fill: { color: PANEL }, line: { color: LINE, width: 1 }, shadow: mkShadow() });
    s.addText(c[0], { x: x + 0.24, y: y + 0.16, w: 3.4, h: 0.26, fontFace: NUM, fontSize: 10, color: LIGHT, charSpacing: 2, margin: 0 });
    s.addText(c[1], { x: x + 0.24, y: y + 0.44, w: 3.4, h: 0.36, fontFace: F, fontSize: 14.5, bold: true, color: TEXT, margin: 0 });
    s.addText(c[2], { x: x + 0.24, y: y + 0.88, w: 3.45, h: 0.95, fontFace: F, fontSize: 11.5, color: MUTED, margin: 0, lineSpacingMultiple: 1.22 });
    s.addText(c[3], { x: x + 0.24, y: y + 1.9, w: 3.45, h: 0.3, fontFace: NUM, fontSize: 12, bold: true, color: SUCCESS, align: 'right', margin: 0 });
  });
  s.addText('诚实口径：召回率 ≥0.95 约束下桌面默认仍是精确索引 —— 引擎价值在存储布局与查询开销（10k p50：SQLite ≈126ms → 引擎 ≈2.2ms），不谈「快 N 倍」。', { x: 0.55, y: 6.85, w: 12.23, h: 0.3, fontFace: F, fontSize: 11, color: DIM, align: 'center', margin: 0 });
  s.addNotes('给技术评委的一页，六个点每个都有测试背书。说两个：第一，向量引擎是自研 Rust crate，有 WAL 和崩溃恢复，63 项测试——但基准我们给诚实口径：召回率约束下桌面默认仍用精确索引，价值在存储布局和查询开销，不是营销数字。第二，AI 失败绝不静默降级成假内容——配置的上游坏了就报错，只有显式离线模式才用明确标注的本地生成。竞赛演示也不能造假。');
})();

// ============================================================ P12 完成度与验证
(() => {
  const s = base('12 · 完成度与验证', '用测试数字说话，用诚实清单收尾', 12);
  s.addText('294', { x: 0.55, y: 1.5, w: 2.2, h: 1.0, fontFace: NUM, fontSize: 54, bold: true, color: LIGHT, margin: 0 });
  s.addText('项自动化测试 · 全部通过', { x: 2.75, y: 2.1, w: 4.5, h: 0.4, fontFace: F, fontSize: 15, bold: true, color: TEXT, margin: 0 });
  const bars = [
    ['前端契约与 UI', 71], ['Rust 核心', 78], ['学习闭环集成', 16],
    ['Provider 与 Cloud', 33], ['向量引擎与检索', 76], ['竞赛路径与审计回归', 20],
  ];
  bars.forEach((b, i) => {
    const y = 2.85 + i * 0.62;
    s.addText(b[0], { x: 0.55, y, w: 2.65, h: 0.32, fontFace: F, fontSize: 12.5, color: TEXT, margin: 0, valign: 'middle' });
    s.addShape(sh.ROUNDED_RECTANGLE, { x: 3.35, y: y + 0.02, w: (b[1] / 78) * 3.9, h: 0.28, rectRadius: 0.14, fill: { color: SUCCESS, transparency: 8 } });
    s.addText(String(b[1]), { x: 3.45 + (b[1] / 78) * 3.9, y, w: 0.8, h: 0.32, fontFace: NUM, fontSize: 13, bold: true, color: LIGHT, margin: 0, valign: 'middle' });
  });
  s.addShape(sh.ROUNDED_RECTANGLE, { x: 8.35, y: 1.55, w: 4.43, h: 5.1, rectRadius: 0.09, fill: { color: PANEL }, line: { color: LINE, width: 1 } });
  s.addText('诚实缺口 · 尚未验证', { x: 8.65, y: 1.82, w: 3.9, h: 0.35, fontFace: F, fontSize: 13.5, bold: true, color: MUTED, margin: 0 });
  const gaps = ['Android：构建与签名就绪，真机验证进行中', '云端 Live E2E：需 Docker 环境，默认 opt-in', 'MSI 打包受预发布号限制；NSIS 为正式产物', '1 个云并发测试并行偶发；串行稳定通过'];
  s.addText(gaps.map((t, i) => ({ text: t, options: { bullet: { code: '25AA', indent: 12 }, breakLine: i < 3 } })), { x: 8.7, y: 2.4, w: 3.85, h: 3.9, fontFace: F, fontSize: 12, color: TEXT, paraSpaceAfter: 14, margin: 0, lineSpacingMultiple: 1.2 });
  s.addText('来源：audit/competition-convergence/COMPETITION_CONVERGENCE_REPORT.md（2026-09-20）· 黄金路径 10 连跑 · 另含 6+12 项审计回归', { x: 0.55, y: 7.05, w: 12.23, h: 0.3, fontFace: F, fontSize: 11, color: DIM, margin: 0 });
  s.addNotes('左边是数字：前端、Rust 核心、学习闭环、向量引擎、云端，将近三百项测试全绿，黄金路径连续跑十遍。右边同样重要——我们没验证什么也写在这里：Android 还没上真机、云端 Live 测试需要现场起 Docker、一个并发测试并行偶发。评委现在检查的每一项，我们都能指出对应的测试文件。可信，比吹牛重要。');
})();

// ============================================================ P13 现场 Demo
(() => {
  const s = base('13 · 现场 Demo', '不是录屏，是可复验的真系统', 13);
  const steps = [
    ['一键重置', '演示路线幂等重建，不动用户数据', false],
    ['Agent 先开口', '提问后展开「依据」，看真实资料出处', false],
    ['故意答错', '路线现场插入「加练 · 无穷远点」', false],
    ['现场断网', '学习继续，界面诚实标注离线', true],
    ['现场重启', '路线、进度、引用、加练都还在', true],
  ];
  steps.forEach((t, i) => {
    const x = 0.55 + i * 2.47, hot = t[2];
    s.addText([
      { text: `${i + 1}`, options: { fontFace: NUM, fontSize: 20, bold: true, color: hot ? LIGHT : MUTED, breakLine: true } },
      { text: t[0], options: { fontSize: 13.5, bold: true, color: TEXT, breakLine: true } },
      { text: t[1], options: { fontSize: 10.5, color: MUTED } },
    ], {
      x, y: 2.0, w: 2.35, h: 1.95, fontFace: F, align: 'left', valign: 'top', margin: 0.14,
      shape: sh.ROUNDED_RECTANGLE, rectRadius: 0.09,
      fill: { color: hot ? '1A2430' : PANEL }, line: { color: hot ? LIGHT : LINE, width: hot ? 1.25 : 1 },
      lineSpacingMultiple: 1.18,
    });
    if (i < 4) s.addText('→', { x: x + 2.33, y: 2.75, w: 0.2, h: 0.4, fontFace: F, fontSize: 16, color: DIM, align: 'center', margin: 0 });
  });
  s.addText('欢迎评委现场出题。', { x: 0.55, y: 4.75, w: 12.23, h: 0.55, fontFace: F, fontSize: 20, bold: true, color: LIGHT, align: 'center', margin: 0 });
  s.addText('应急方案内置：一键重置 · 离线降级 · 全新 profile ｜ 完整脚本见 docs/COMPETITION_DEMO.md', { x: 0.55, y: 5.5, w: 12.23, h: 0.35, fontFace: F, fontSize: 12.5, color: MUTED, align: 'center', margin: 0 });
  s.addNotes('Demo 不是录屏。我们当场做五件事，其中两件很多项目不敢做：当场拔网——学习继续，界面明说离线；当场重启——刚才答错长出来的加练节点还在。因为它不是演示脚本，是一个本地优先的真系统。也欢迎评委现场提一个问题，让 Agent 现场答。');
})();

// ============================================================ P14 收尾
(() => {
  const s = p.addSlide();
  s.background = { color: BG };
  lighthouse(s, 10.55, 2.55, 1.0, { fog: 2 });
  const pts = [[0.95, 6.15], [2.15, 5.8], [3.35, 6.1], [4.55, 5.7], [5.75, 6.0]];
  for (let i = 0; i < 4; i++) s.addShape(sh.LINE, { x: pts[i][0] + 0.12, y: Math.min(pts[i][1], pts[i + 1][1]) + 0.06, w: pts[i + 1][0] - pts[i][0] - 0.1, h: Math.abs(pts[i + 1][1] - pts[i][1]), flipV: pts[i + 1][1] < pts[i][1], line: { color: DIM, width: 1.2, dashType: 'dash' } });
  pts.forEach((pt, i) => nodeDot(s, pt[0], pt[1], i < 3 ? 'lit' : i === 3 ? 'current' : 'unlit'));
  s.addText('AI 已经很会回答问题了。', { x: 0.9, y: 2.35, w: 8.5, h: 0.7, fontFace: F, fontSize: 24, color: MUTED, margin: 0 });
  s.addText('我们想让它开始学会怎样教人。', { x: 0.9, y: 3.15, w: 9.5, h: 0.9, fontFace: F, fontSize: 34, bold: true, color: LIGHT, margin: 0 });
  s.addText('学涯 Orbit · 谢谢聆听 · 欢迎提问与现场体验', { x: 0.9, y: 6.75, w: 9, h: 0.35, fontFace: F, fontSize: 13, color: MUTED, margin: 0 });
  s.addNotes('（Demo 之后回到这一页）这就是我们全部想做的事：AI 已经很会回答问题了，学涯想让它开始学会怎样教人。谢谢各位，欢迎提问。');
})();

p.writeFile({ fileName: 'H:/项目/CrossPlatform/学涯/competition/学涯Orbit-竞赛路演.pptx' }).then(() => console.log('OK'));
