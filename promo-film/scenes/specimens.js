/* Specimen cards — procedural 19c-archive line art for the ARCHIVE montage.
 * Each renderer draws onto an offscreen canvas in one consistent engraving
 * hand: 1px graphite linework, paper ground, a specimen stamp.
 * All randomness is seeded per card. */
"use strict";

const SPEC = (() => {

const PAPER = "#f6f2ea";
const INK = "#2a2c30";
const FAINT = "#8a8578";
const SIGNAL = "#35d0a5";

function base(w, h, seed) {
  const c = document.createElement("canvas");
  c.width = w; c.height = h;
  const g = c.getContext("2d");
  g.fillStyle = PAPER;
  g.fillRect(0, 0, w, h);
  // paper fibre: faint horizontal streaks + grain
  const rng = mulberry32(seed);
  g.globalAlpha = 0.05;
  for (let i = 0; i < 70; i++) {
    g.strokeStyle = rng() > 0.5 ? "#b8b0a0" : "#d8d2c4";
    g.lineWidth = 1;
    const y = rng() * h;
    g.beginPath(); g.moveTo(0, y); g.lineTo(w, y + (rng() - 0.5) * 8); g.stroke();
  }
  g.globalAlpha = 0.06;
  for (let i = 0; i < 1400; i++) {
    g.fillStyle = rng() > 0.5 ? "#7a7466" : "#cfc8b8";
    g.fillRect(rng() * w, rng() * h, 1.2, 1.2);
  }
  g.globalAlpha = 1;
  g.strokeStyle = INK;
  g.lineWidth = 1;
  g.strokeRect(14.5, 14.5, w - 29, h - 29);
  g.strokeRect(20.5, 20.5, w - 41, h - 41);
  return { c, g, rng };
}

function stamp(g, text, x, y, color) {
  g.save();
  g.font = "600 15px 'Cascadia Mono','Consolas',monospace";
  g.letterSpacing = "3px";
  g.fillStyle = color || FAINT;
  g.fillText(text, x, y);
  g.restore();
}

function label(g, text, x, y) {
  g.save();
  g.font = "13px 'Cascadia Mono','Consolas',monospace";
  g.fillStyle = FAINT;
  g.fillText(text, x, y);
  g.restore();
}

/* wobbly hand line */
function handLine(g, rng, x1, y1, x2, y2, wob) {
  const steps = Math.max(2, Math.floor(Math.hypot(x2 - x1, y2 - y1) / 14));
  g.beginPath();
  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    const x = lerp(x1, x2, t) + (rng() - 0.5) * (wob || 1.6);
    const y = lerp(y1, y2, t) + (rng() - 0.5) * (wob || 1.6);
    i ? g.lineTo(x, y) : g.moveTo(x, y);
  }
  g.stroke();
}

/* ---- 月面图 selenography ---- */
function moonChart() {
  const { c, g, rng } = base(640, 640, 101);
  const cx = 320, cy = 310, R = 210;
  g.strokeStyle = INK; g.lineWidth = 1.4;
  g.beginPath(); g.arc(cx, cy, R, 0, Math.PI * 2); g.stroke();
  // terminator shade (hatching on the dark limb)
  g.save();
  g.beginPath(); g.arc(cx, cy, R, 0, Math.PI * 2); g.clip();
  for (let i = -R; i < R; i += 5) {
    const x = cx - R * 0.45 + i;
    g.globalAlpha = 0.35;
    handLine(g, rng, x, cy - Math.sqrt(Math.max(0, R * R - (x - cx) ** 2)), x + 14, cy + Math.sqrt(Math.max(0, R * R - (x - cx) ** 2)), 0.8);
  }
  g.globalAlpha = 1;
  // craters
  for (let i = 0; i < 26; i++) {
    const a = rng() * Math.PI * 2, r = Math.sqrt(rng()) * R * 0.86;
    const x = cx + Math.cos(a) * r, y = cy + Math.sin(a) * r;
    const cr = 3 + rng() * 16;
    g.beginPath(); g.arc(x, y, cr, 0, Math.PI * 2); g.stroke();
    g.beginPath(); g.arc(x - cr * 0.25, y - cr * 0.25, cr * 0.55, Math.PI * 0.9, Math.PI * 1.7); g.stroke();
  }
  // graticule
  g.globalAlpha = 0.4;
  for (let i = 1; i < 4; i++) { g.beginPath(); g.ellipse(cx, cy, R, (R * i) / 4, 0, 0, Math.PI * 2); g.stroke(); }
  for (let i = 1; i < 4; i++) { g.beginPath(); g.ellipse(cx, cy, (R * i) / 4, R, 0, 0, Math.PI * 2); g.stroke(); }
  g.restore();
  g.globalAlpha = 1;
  label(g, "MARE TRANQUILLITATIS", cx + 40, cy - 60);
  handLine(g, rng, cx + 34, cy - 56, cx + 90, cy - 20, 0.6);
  label(g, "α = 23.4°", cx - 190, cy + 150);
  stamp(g, "PLATE IX · SELENOGRAPHY", 48, 596);
  return c;
}

/* ---- 潮汐表 tide table ---- */
function tideTable() {
  const { c, g, rng } = base(640, 640, 202);
  stamp(g, "TIDE TABLE · 52°N 4°E", 48, 70);
  // 24h sine water levels
  const x0 = 70, x1 = 590, yMid = 300, amp = 120;
  g.strokeStyle = INK; g.lineWidth = 1;
  g.beginPath(); g.moveTo(x0, yMid); g.lineTo(x1, yMid); g.stroke();
  for (let i = 0; i <= 24; i += 2) {
    const x = lerp(x0, x1, i / 24);
    g.beginPath(); g.moveTo(x, yMid - 5); g.lineTo(x, yMid + 5); g.stroke();
    g.font = "11px 'Cascadia Mono',monospace"; g.fillStyle = FAINT;
    g.fillText(String(i).padStart(2, "0") + ":00", x - 16, yMid + 170);
  }
  g.lineWidth = 1.6; g.strokeStyle = INK;
  g.beginPath();
  for (let x = x0; x <= x1; x += 3) {
    const t = (x - x0) / (x1 - x0);
    const y = yMid - Math.sin(t * Math.PI * 4) * amp * (0.82 + 0.18 * Math.sin(t * 9));
    x === x0 ? g.moveTo(x, y) : g.lineTo(x, y);
  }
  g.stroke();
  // high/low water marks
  g.setLineDash([4, 4]); g.globalAlpha = 0.55;
  g.beginPath(); g.moveTo(x0, yMid - amp); g.lineTo(x1, yMid - amp); g.stroke();
  g.beginPath(); g.moveTo(x0, yMid + amp); g.lineTo(x1, yMid + amp); g.stroke();
  g.setLineDash([]); g.globalAlpha = 1;
  label(g, "HW +4.2m", x1 - 90, yMid - amp - 8);
  label(g, "LW +0.4m", x1 - 90, yMid + amp + 18);
  // moon phase row
  for (let i = 0; i < 8; i++) {
    const x = x0 + 30 + i * 62, y = 520, r = 13;
    g.beginPath(); g.arc(x, y, r, 0, Math.PI * 2); g.stroke();
    g.save(); g.beginPath(); g.arc(x, y, r, 0, Math.PI * 2); g.clip();
    g.fillStyle = INK; g.globalAlpha = 0.75;
    g.fillRect(x - r + (i / 7) * 2 * r, y - r, 2 * r, 2 * r);
    g.restore(); g.globalAlpha = 1;
  }
  stamp(g, "TABLE II · LUNAR DAY", 48, 596);
  return c;
}

/* ---- 鸟翼标本 bird wing ---- */
function birdWing() {
  const { c, g, rng } = base(640, 640, 303);
  stamp(g, "SPECIMEN 07 · ALA", 48, 70);
  const ox = 130, oy = 380;
  g.strokeStyle = INK;
  // arm bones
  g.lineWidth = 2;
  handLine(g, rng, ox, oy, ox + 190, oy - 120, 2);
  handLine(g, rng, ox + 190, oy - 120, ox + 380, oy - 150, 2);
  // feather fan
  g.lineWidth = 1;
  for (let i = 0; i < 22; i++) {
    const t = i / 21;
    const bx = ox + 190 + t * 190, by = oy - 120 - t * 30;
    const len = 150 - t * 90;
    const ang = -0.35 - t * 0.5;
    g.globalAlpha = 0.5 + 0.5 * (1 - t);
    handLine(g, rng, bx, by, bx + Math.cos(ang) * len * 1.9, by + Math.sin(ang) * len * 1.9 + 60 * (1 - t), 1.2);
  }
  g.globalAlpha = 1;
  // covert rows
  for (let r = 0; r < 3; r++) {
    for (let i = 0; i < 12 - r * 2; i++) {
      const x = ox + 120 + i * 26 + r * 16, y = oy - 80 - r * 26 - i * 3;
      g.beginPath(); g.arc(x, y, 12 - r * 2, Math.PI * 0.15, Math.PI * 0.95); g.stroke();
    }
  }
  // scale bar
  handLine(g, rng, 70, 560, 190, 560, 0.5);
  handLine(g, rng, 70, 554, 70, 566, 0.3); handLine(g, rng, 190, 554, 190, 566, 0.3);
  label(g, "10 cm", 116, 580);
  label(g, "remiges primariae", 330, 180);
  handLine(g, rng, 400, 190, 470, 235, 0.6);
  return c;
}

/* ---- 几何证明 geometry proof ---- */
function geoProof() {
  const { c, g, rng } = base(640, 640, 404);
  const cx = 300, cy = 300, R = 170;
  g.strokeStyle = INK; g.lineWidth = 1.4;
  g.beginPath(); g.arc(cx, cy, R, 0, Math.PI * 2); g.stroke();
  // inscribed triangle + chords
  const A = { x: cx - R * 0.86, y: cy + R * 0.5 }, B = { x: cx + R * 0.99, y: cy - R * 0.14 }, C = { x: cx - R * 0.2, y: cy - R * 0.98 };
  const D = { x: cx + R * 0.3, y: cy + R * 0.95 };
  [[A, B], [B, C], [C, A], [A, D], [B, D]].forEach(([p, q]) => handLine(g, rng, p.x, p.y, q.x, q.y, 1.2));
  // angle arcs
  g.beginPath(); g.arc(A.x, A.y, 34, -0.9, 0.25); g.stroke();
  g.beginPath(); g.arc(D.x, D.y, 26, Math.PI * 1.1, Math.PI * 1.7); g.stroke();
  g.font = "italic 22px 'Palatino Linotype',serif"; g.fillStyle = INK;
  g.fillText("A", A.x - 22, A.y + 12); g.fillText("B", B.x + 10, B.y + 4);
  g.fillText("C", C.x - 4, C.y - 12); g.fillText("D", D.x + 6, D.y + 26);
  g.fillText("α", A.x + 20, A.y - 12); g.fillText("β", D.x - 12, D.y - 14);
  g.font = "italic 19px 'Palatino Linotype',serif";
  g.fillText("(AB,CD) = (AC·BD)/(BC·AD)", 90, 560);
  g.font = "15px 'Palatino Linotype',serif"; g.fillStyle = FAINT;
  g.fillText("∴ invariable under projection.  Q.E.D.", 90, 588);
  stamp(g, "PROP. 14 · CROSS RATIO", 48, 70);
  return c;
}

/* ---- PDF 页与批注 annotated page ---- */
function pdfPage() {
  const { c, g, rng } = base(640, 760, 505);
  stamp(g, "ARCHIVE / PDF · p.113", 48, 66);
  // heading
  g.fillStyle = INK;
  g.font = "700 24px 'Palatino Linotype',serif";
  g.fillText("§3.2  Die projective Ebene", 60, 120);
  // text lines
  g.strokeStyle = INK;
  let y = 160;
  const rows = [];
  while (y < 560) { rows.push(y); y += 21; }
  rows.forEach((yy, i) => {
    let x = 60;
    const end = i % 7 === 6 ? 440 : 580;
    while (x < end) {
      const w = 14 + rng() * 34;
      g.globalAlpha = 0.6;
      g.beginPath(); g.moveTo(x, yy); g.lineTo(Math.min(x + w, end), yy); g.stroke();
      x += w + 9;
    }
  });
  g.globalAlpha = 1;
  // red-ish pencil annotation (use warm terracotta to stay in palette)
  g.strokeStyle = "#b65f41"; g.lineWidth = 1.6;
  g.beginPath(); g.ellipse(300, 286, 190, 30, -0.02, 0, Math.PI * 2); g.stroke();
  g.font = "italic 20px 'Palatino Linotype',serif"; g.fillStyle = "#b65f41";
  g.save(); g.rotate(-0.02);
  g.fillText("invariant! → key", 370, 340);
  g.restore();
  handLine(g, rng, 372, 330, 340, 300, 0.8);
  // margin note
  g.font = "15px 'Cascadia Mono',monospace"; g.fillStyle = FAINT;
  g.fillText("marginalia, 1987", 60, 620);
  return c;
}

/* ---- 等高线地形图 contour map ---- */
function contourMap() {
  const { c, g, rng } = base(760, 640, 606);
  stamp(g, "SURVEY SHEET 22 · RELIEF", 48, 70);
  g.strokeStyle = INK;
  for (let k = 0; k < 12; k++) {
    g.lineWidth = k % 4 === 0 ? 1.5 : 0.8;
    g.globalAlpha = 0.5 + 0.5 * (k % 4 === 0 ? 1 : 0.6);
    g.beginPath();
    const baseR = 40 + k * 26;
    for (let a = 0; a <= Math.PI * 2 + 0.01; a += 0.08) {
      const r = baseR * (1 + 0.35 * fbm(Math.cos(a) * 1.4 + k * 0.7, Math.sin(a) * 1.4 + 3, 3));
      const x = 380 + Math.cos(a) * r * 1.25, y = 330 + Math.sin(a) * r * 0.9;
      a === 0 ? g.moveTo(x, y) : g.lineTo(x, y);
    }
    g.closePath(); g.stroke();
  }
  g.globalAlpha = 1;
  // second hill
  for (let k = 0; k < 6; k++) {
    g.lineWidth = 0.8; g.globalAlpha = 0.6;
    g.beginPath();
    for (let a = 0; a <= Math.PI * 2 + 0.01; a += 0.1) {
      const r = (24 + k * 18) * (1 + 0.3 * fbm(Math.cos(a) + 9 + k, Math.sin(a) + 1, 3));
      const x = 610 + Math.cos(a) * r, y = 180 + Math.sin(a) * r * 0.8;
      a === 0 ? g.moveTo(x, y) : g.lineTo(x, y);
    }
    g.closePath(); g.stroke();
  }
  g.globalAlpha = 1;
  label(g, "1420", 372, 200); label(g, "1360", 400, 300);
  // route dashed across
  g.strokeStyle = "#b65f41"; g.lineWidth = 1.6; g.setLineDash([7, 6]);
  g.beginPath();
  for (let i = 0; i <= 40; i++) {
    const t = i / 40;
    const x = lerp(90, 690, t), y = 480 - 260 * t + Math.sin(t * 7) * 30;
    i === 0 ? g.moveTo(x, y) : g.lineTo(x, y);
  }
  g.stroke(); g.setLineDash([]);
  label(g, "survey route", 620, 480);
  return c;
}

/* ---- 代码片段 code fragment ---- */
function codeFragment() {
  const { c, g } = base(760, 640, 707);
  stamp(g, "LISTING 3 · ADAPT", 48, 70);
  const lines = [
    ["fn ", "adapt_route", "(state: &Learner) {"],
    ["  let ", "mastery", " = state.mastery();"],
    ["  if ", "misconception.detected() ", "{"],
    ["    route.", "insert_review", "(node, reason);"],
    ["    ", "// the map changes, not the learner's pace", ""],
    ["  }"],
    ["  ", "ask", "(probe::WHY)"],
    ["}", ""],
  ];
  g.font = "19px 'Cascadia Mono','Consolas',monospace";
  let y = 130;
  g.fillStyle = FAINT;
  lines.forEach((ln, i) => {
    g.fillStyle = "#a39e90";
    g.fillText(String(i + 1).padStart(2, " "), 52, y);
    g.fillStyle = INK;
    g.fillText(ln.join(""), 92, y);
    y += 42;
  });
  g.strokeStyle = INK; g.lineWidth = 1;
  g.strokeRect(76.5, 96.5, 640, 8 * 42 + 14);
  return c;
}

/* ---- 电路板走线 circuit traces ---- */
function circuitBoard() {
  const { c, g, rng } = base(760, 640, 808);
  stamp(g, "PCB · REV C", 48, 70);
  g.strokeStyle = INK;
  for (let i = 0; i < 26; i++) {
    let x = 60 + rng() * 620, y = 100 + rng() * 440;
    g.lineWidth = 1.4;
    g.beginPath(); g.moveTo(x, y);
    for (let s = 0; s < 4; s++) {
      const dx = (rng() - 0.3) * 130, dy = (rng() - 0.5) * 40;
      x += dx; g.lineTo(x, y); y += dy; g.lineTo(x, y);
    }
    g.stroke();
    g.beginPath(); g.arc(x, y, 5, 0, Math.PI * 2); g.stroke();
  }
  for (let i = 0; i < 9; i++) {
    const x = 90 + rng() * 560, y = 120 + rng() * 400;
    g.strokeRect(x, y, 26 + rng() * 30, 14 + rng() * 10);
  }
  return c;
}

/* ---- 星图 star chart ---- */
function starChart() {
  const { c, g, rng } = base(760, 640, 909);
  // dark plate — star charts print white on ink
  g.fillStyle = "#14181d"; g.fillRect(20, 20, 720, 600);
  g.strokeStyle = "#e8e4dc"; g.globalAlpha = 0.9;
  const pts = [];
  for (let i = 0; i < 46; i++) {
    const x = 50 + rng() * 660, y = 50 + rng() * 540;
    const r = rng() < 0.16 ? 2.6 : 1.2;
    pts.push({ x, y });
    g.globalAlpha = 0.35 + rng() * 0.6;
    g.beginPath(); g.arc(x, y, r, 0, Math.PI * 2); g.fill();
  }
  g.fillStyle = "#e8e4dc";
  // constellation
  g.globalAlpha = 0.8; g.lineWidth = 0.9; g.strokeStyle = "#e8e4dc";
  g.setLineDash([2, 5]);
  g.beginPath();
  [3, 7, 12, 15, 21, 28].forEach((idx, i) => {
    const p = pts[idx];
    i === 0 ? g.moveTo(p.x, p.y) : g.lineTo(p.x, p.y);
  });
  g.stroke(); g.setLineDash([]);
  // declination circles
  g.globalAlpha = 0.3;
  for (let i = 1; i <= 3; i++) { g.beginPath(); g.arc(380, 320, i * 150, 0, Math.PI * 2); g.stroke(); }
  g.globalAlpha = 1;
  g.fillStyle = "#e8e4dc";
  stamp(g, "CHART VI · NORTHERN SKY", 48, 596, "#e8e4dc");
  return c;
}

return { moonChart, tideTable, birdWing, geoProof, pdfPage, contourMap, codeFragment, circuitBoard, starChart };
})();
