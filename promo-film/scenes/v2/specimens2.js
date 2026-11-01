/* Specimen cards v2 — six archival plates, all about ONE question:
 * "Why do parallel rails meet at the horizon?"
 * Each card carries one catalog keyword box (the extraction target).
 * kw rect is normalized card coordinates. */
"use strict";

const SPEC2 = (() => {

const PAPER = "#f6f2ea";
const INK = "#2a2c30";
const FAINT = "#8a8578";
const RED = "#b65f41";

function base(w, h, seed) {
  const c = document.createElement("canvas");
  c.width = w; c.height = h;
  const g = c.getContext("2d");
  g.fillStyle = PAPER;
  g.fillRect(0, 0, w, h);
  const rng = mulberry32(seed);
  g.globalAlpha = 0.05;
  for (let i = 0; i < 60; i++) {
    g.strokeStyle = rng() > 0.5 ? "#b8b0a0" : "#d8d2c4";
    g.beginPath(); g.moveTo(0, rng() * h); g.lineTo(w, rng() * h); g.stroke();
  }
  g.globalAlpha = 0.07;
  for (let i = 0; i < 1100; i++) {
    g.fillStyle = rng() > 0.5 ? "#7a7466" : "#cfc8b8";
    g.fillRect(rng() * w, rng() * h, 1.2, 1.2);
  }
  g.globalAlpha = 1;
  g.strokeStyle = INK; g.lineWidth = 1;
  g.strokeRect(14.5, 14.5, w - 29, h - 29);
  g.strokeRect(20.5, 20.5, w - 41, h - 41);
  return { c, g, rng };
}

function mono(g, text, x, y, px, color, align) {
  g.save();
  g.font = `500 ${px || 14}px 'Cascadia Mono','Consolas',monospace`;
  try { g.letterSpacing = "2.5px"; } catch (e) {}
  g.fillStyle = color || FAINT;
  g.textAlign = align || "left";
  g.fillText(text, x, y);
  g.restore();
}

function zh(g, text, x, y, px, color) {
  g.save();
  g.font = `400 ${px || 18}px 'SimSun','Songti SC',serif`;
  g.fillStyle = color || INK;
  g.fillText(text, x, y);
  g.restore();
}

/* catalog keyword box — the extraction target */
function kwBox(g, text, sub, cx, cy, w, h) {
  g.save();
  g.strokeStyle = INK;
  g.lineWidth = 1.2;
  g.strokeRect(cx - w / 2, cy - h / 2, w, h);
  g.fillStyle = "rgba(255,254,250,0.85)";
  g.fillRect(cx - w / 2, cy - h / 2, w, h);
  g.strokeRect(cx - w / 2, cy - h / 2, w, h);
  zh(g, text, cx - w / 2 + 12, cy + 2, 19, INK);
  mono(g, sub, cx - w / 2 + 12, cy + 20, 11, FAINT);
  g.restore();
}

function handLine(g, rng, x1, y1, x2, y2, wob) {
  const steps = Math.max(2, Math.floor(Math.hypot(x2 - x1, y2 - y1) / 14));
  g.beginPath();
  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    const x = lerp(x1, x2, t) + (rng() - 0.5) * (wob || 1.4);
    const y = lerp(y1, y2, t) + (rng() - 0.5) * (wob || 1.4);
    i ? g.lineTo(x, y) : g.moveTo(x, y);
  }
  g.stroke();
}

/* 1 — 透视习作：收拢于正交线的消失点 → 射影平面 */
function perspectiveStudy() {
  const { c, g, rng } = base(760, 600, 21);
  const vpx = 380, vpy = 220;
  mono(g, "FIG. 1 · PERSPECTIVE STUDY", 40, 60, 14);
  // horizon
  g.lineWidth = 1.4; g.strokeStyle = INK;
  handLine(g, rng, 60, vpy, 700, vpy, 0.8);
  mono(g, "horizon", 640, vpy - 10, 12);
  // orthogonals from bottom edge to VP
  g.lineWidth = 1;
  for (let i = 0; i <= 10; i++) {
    const x = 80 + i * 62;
    g.globalAlpha = 0.75;
    handLine(g, rng, x, 540, vpx, vpy, 1);
  }
  g.globalAlpha = 1;
  // telegraph poles shrinking
  for (let i = 0; i < 6; i++) {
    const t = i / 5;
    const x = lerp(140, vpx - 8, Math.pow(t, 0.72));
    const yb = lerp(540, vpy, Math.pow(t, 0.72));
    const hh = lerp(150, 6, Math.pow(t, 0.72));
    g.lineWidth = 2;
    handLine(g, rng, x, yb, x, yb - hh, 0.7);
    g.lineWidth = 1;
    handLine(g, rng, x - hh * 0.18, yb - hh * 0.8, x + hh * 0.18, yb - hh * 0.8, 0.4);
  }
  // VP mark
  g.strokeStyle = RED; g.lineWidth = 1.4;
  g.beginPath(); g.arc(vpx, vpy, 10, 0, Math.PI * 2); g.stroke();
  g.beginPath(); g.moveTo(vpx - 16, vpy); g.lineTo(vpx + 16, vpy); g.moveTo(vpx, vpy - 16); g.lineTo(vpx, vpy + 16); g.stroke();
  mono(g, "V.P.", vpx + 20, vpy + 4, 12, RED);
  kwBox(g, "射影平面", "PROJECTIVE PLANE", 176, 516, 220, 52);
  return { cv: c, kw: { x0: 0.087, y0: 0.817, x1: 0.376, y1: 0.903 }, kwText: "射影平面" };
}

/* 2 — 铁轨图版：平行线 + 无穷远坐标 → 齐次坐标 */
function railwayPlate() {
  const { c, g, rng } = base(760, 600, 22);
  const vpx = 400, vpy = 200;
  mono(g, "PLATE II · THE PARALLEL CASE", 40, 60, 14);
  // sky hatch
  g.globalAlpha = 0.25;
  for (let i = 0; i < 16; i++) handLine(g, rng, 60, 80 + i * 7, 700, 74 + i * 7, 0.8);
  g.globalAlpha = 1;
  // rails
  g.lineWidth = 2;
  handLine(g, rng, 180, 540, vpx - 4, vpy, 0.8);
  handLine(g, rng, 620, 540, vpx + 4, vpy, 0.8);
  // sleepers
  g.lineWidth = 1.4;
  for (let i = 0; i < 11; i++) {
    const t = Math.pow(i / 10, 1.9);
    const y = lerp(536, vpy + 2, t);
    const xl = lerp(180, vpx - 4, t), xr = lerp(620, vpx + 4, t);
    handLine(g, rng, xl, y, xr, y, 0.5);
  }
  // coordinate annotation: a sleeper with (x,y) and the VP with (x,y,0)
  mono(g, "(x, y, 1)", 500, 470, 13, INK);
  handLine(g, rng, 520, 462, 478, 448, 0.4);
  g.strokeStyle = RED;
  mono(g, "(x, y, 0) — at infinity", vpx - 190, vpy - 26, 13, RED);
  handLine(g, rng, vpx - 12, vpy - 20, vpx - 2, vpy - 4, 0.4);
  kwBox(g, "齐次坐标", "HOMOGENEOUS COORDS", 570, 516, 250, 52);
  return { cv: c, kw: { x0: 0.586, y0: 0.817, x1: 0.915, y1: 0.903 }, kwText: "齐次坐标" };
}

/* 3 — 公式页：交比定义 → 交比 */
function formulaPage() {
  const { c, g, rng } = base(620, 760, 23);
  mono(g, "DE RATIONE · III", 40, 66, 14);
  g.font = "italic 700 30px 'Palatino Linotype',serif";
  g.fillStyle = INK;
  g.fillText("On the cross ratio", 44, 130);
  // four collinear points
  g.strokeStyle = INK; g.lineWidth = 1.6;
  handLine(g, rng, 60, 260, 560, 260, 0.8);
  ["A", "B", "C", "D"].forEach((L, i) => {
    const x = 110 + i * 120;
    g.beginPath(); g.arc(x, 260, 4, 0, Math.PI * 2); g.fillStyle = INK; g.fill();
    g.font = "italic 24px 'Palatino Linotype',serif";
    g.fillText(L, x - 6, 296);
  });
  // braces showing AC, BD etc.
  g.lineWidth = 1; g.globalAlpha = 0.6;
  g.beginPath(); g.moveTo(110, 236); g.quadraticCurveTo(170, 210, 350, 236); g.stroke();
  g.globalAlpha = 1;
  // the formula, large
  g.font = "italic 400 34px 'Palatino Linotype',serif";
  g.fillText("(AB,CD) = ", 70, 420);
  g.fillText("AC·BD", 260, 396); g.fillText("BC·AD", 260, 452);
  g.beginPath(); g.moveTo(250, 424); g.lineTo(380, 424); g.stroke();
  g.font = "italic 19px 'Palatino Linotype',serif"; g.fillStyle = FAINT;
  g.fillText("invariable under every projection", 70, 510);
  g.font = "italic 19px 'Palatino Linotype',serif";
  g.fillText("harmonic when equal to −1", 70, 540);
  kwBox(g, "交比", "CROSS RATIO", 180, 650, 200, 52);
  return { cv: c, kw: { x0: 0.129, y0: 0.821, x1: 0.452, y1: 0.889 }, kwText: "交比" };
}

/* 4 — 对偶证明：点↔线 → 对偶原理 */
function dualityProof() {
  const { c, g, rng } = base(620, 760, 24);
  mono(g, "PROP. 9 · TWO COLUMNS", 40, 66, 14);
  // two columns: point statements | line statements
  g.lineWidth = 1; g.strokeStyle = INK;
  handLine(g, rng, 310, 100, 310, 560, 0.8);
  mono(g, "POINT", 120, 130, 15, INK, "center");
  mono(g, "LINE", 500, 130, 15, INK, "center");
  const pairs = [
    ["two points", "two lines"],
    ["determine", "determine"],
    ["one line", "one point"],
  ];
  pairs.forEach((row, i) => {
    const y = 210 + i * 110;
    zh(g, row[0], 92, y, 19, INK); mono(g, row[1] === "two lines" ? "" : "", 0, 0);
    zh(g, row[1], 412, y, 19, INK);
    g.globalAlpha = 0.5;
    handLine(g, rng, 200, y - 6, 400, y - 6, 0.6);
    g.globalAlpha = 1;
    mono(g, "⇄", 292, y + 2, 20, RED, "center");
  });
  // small dual diagram: point with pencil of lines / line with range of points
  g.lineWidth = 1;
  g.beginPath(); g.arc(140, 520, 5, 0, Math.PI * 2); g.fill();
  for (let i = 0; i < 5; i++) { const a = -0.5 + i * 0.32; handLine(g, rng, 140, 520, 140 + Math.cos(a) * 90, 520 + Math.sin(a) * 90, 0.5); }
  handLine(g, rng, 420, 545, 520, 490, 0.5);
  for (let i = 0; i < 5; i++) { const t = i / 4; g.beginPath(); g.arc(lerp(420, 520, t), lerp(545, 490, t), 3.2, 0, Math.PI * 2); g.fill(); }
  kwBox(g, "对偶原理", "DUALITY", 180, 680, 200, 52);
  return { cv: c, kw: { x0: 0.129, y0: 0.855, x1: 0.452, y1: 0.924 }, kwText: "对偶原理" };
}

/* 5 — 圆锥截面 → 二次曲线 */
function conicSection() {
  const { c, g, rng } = base(620, 760, 25);
  mono(g, "PLATE V · SECTIONS OF A CONE", 40, 66, 14);
  const apex = { x: 310, y: 140 };
  g.strokeStyle = INK; g.lineWidth = 1.4;
  // cone
  handLine(g, rng, apex.x, apex.y, 180, 430, 1);
  handLine(g, rng, apex.x, apex.y, 440, 430, 1);
  g.beginPath(); g.ellipse(310, 430, 130, 34, 0, 0, Math.PI * 2); g.stroke();
  g.beginPath(); g.ellipse(310, 430, 130, 34, 0, 0, Math.PI); g.stroke();
  // cutting plane
  g.save();
  g.strokeStyle = RED; g.lineWidth = 1.4; g.setLineDash([8, 6]);
  g.beginPath(); g.ellipse(310, 330, 105, 22, -0.06, 0, Math.PI * 2); g.stroke();
  g.restore();
  mono(g, "section", 424, 300, 12, RED);
  // resulting curves
  g.strokeStyle = INK; g.lineWidth = 1.4;
  g.beginPath(); g.ellipse(150, 600, 90, 40, -0.1, 0, Math.PI * 2); g.stroke();
  mono(g, "ellipse", 128, 668, 12);
  g.beginPath(); g.moveTo(360, 640); g.quadraticCurveTo(430, 520, 500, 640); g.stroke();
  mono(g, "parabola", 408, 668, 12);
  kwBox(g, "二次曲线", "CONICS", 180, 700, 200, 52);
  return { cv: c, kw: { x0: 0.129, y0: 0.887, x1: 0.452, y1: 0.955 }, kwText: "二次曲线" };
}

/* 6 — 教科书页与批注：极线与极点 → 配极变换 */
function polarPage() {
  const { c, g, rng } = base(620, 760, 26);
  mono(g, "TEXTBOOK · p.87", 40, 66, 14);
  g.font = "700 24px 'Palatino Linotype',serif";
  g.fillStyle = INK;
  g.fillText("§7  Pole and Polar", 44, 120);
  g.strokeStyle = INK;
  let y = 160;
  while (y < 420) {
    let x = 44;
    const end = 560;
    while (x < end) {
      const w = 12 + rng() * 36;
      g.globalAlpha = 0.55;
      g.beginPath(); g.moveTo(x, y); g.lineTo(Math.min(x + w, end), y); g.stroke();
      x += w + 8;
    }
    y += 20;
  }
  g.globalAlpha = 1;
  // margin sketch: circle, external point P, polar line
  g.lineWidth = 1.3;
  g.beginPath(); g.arc(200, 540, 70, 0, Math.PI * 2); g.stroke();
  g.beginPath(); g.arc(370, 470, 4.5, 0, Math.PI * 2); g.fillStyle = INK; g.fill();
  g.font = "italic 20px 'Palatino Linotype',serif"; g.fillText("P", 380, 462);
  handLine(g, rng, 350, 490, 268, 470, 0.5);
  handLine(g, rng, 350, 490, 240, 570, 0.5);
  g.strokeStyle = RED; g.lineWidth = 1.5;
  handLine(g, rng, 150, 640, 300, 470, 0.6);
  mono(g, "polar of P", 96, 668, 12, RED);
  // pencil note
  g.fillStyle = RED; g.font = "italic 19px 'Palatino Linotype',serif";
  g.fillText("the point remembers its line!", 300, 200);
  kwBox(g, "配极变换", "POLARITY", 180, 700, 200, 52);
  return { cv: c, kw: { x0: 0.129, y0: 0.887, x1: 0.452, y1: 0.955 }, kwText: "配极变换" };
}

function buildAll() {
  return [
    perspectiveStudy(),
    railwayPlate(),
    formulaPage(),
    dualityProof(),
    conicSection(),
    polarPage(),
  ];
}

return { buildAll };
})();
