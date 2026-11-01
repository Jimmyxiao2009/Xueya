/* ORBIT promo v2 — causal-continuity engine, overlap retime (54s).
 * Rule: nothing ever stands still at a boundary. Each action starts while
 * the previous one is still settling; shared transition elements (pulse ring,
 * scan line, expansion ring) are drawn by BOTH neighboring scenes from the
 * same time-based formula, so motion crosses scene cuts seamlessly.
 *
 *   S1 QUESTION        0.0 – 3.6
 *   S2 TOO MUCH        3.4 – 9.6   (pulse starts inside S1)
 *   S3 STRUCTURE       9.6 – 15.6  (scan line converges into the first ring)
 *   S4 ROUTE          15.6 – 20.4
 *   S5 ENTER PRODUCT  20.3 – 25.2  (ring starts inside S4)
 *   S6 LEARNING       25.0 – 34.4
 *   S7 UNDERSTANDING  34.4 – 38.6
 *   S8 EXPAND         38.6 – 46.0
 *   S9 ORBIT          46.0 – 54.0
 */
"use strict";

const W = 1920, H = 1080, FPS = 30, DUR = 54;
const NO_TEXT = new URLSearchParams(location.search).has("noText");

const C = {
  ink: "#1b1d20", paper: "#f6f2ea", warmWhite: "#fffefa",
  fogGrey: "#9aa5a8", sky: "#bccdd6",
  signal: "#35d0a5", signalDim: "rgba(53,208,165,0.55)",
  amber: "#e6c084", dark0: "#06080a", dark1: "#0d1114", chart: "#0b0e12",
};
const F = {
  serif: (px, it) => `${it ? "italic " : ""}400 ${px}px "Palatino Linotype","Book Antiqua",serif`,
  song: (px, w) => `${w || 400} ${px}px "SimSun","Songti SC","STSong",serif`,
  mono: (px, w) => `${w || 500} ${px}px "Cascadia Mono","Consolas",monospace`,
};

/* motion design: fast attack, hard settle. Everything physical accelerates. */
const E = {
  outQuint: (t) => 1 - Math.pow(1 - t, 5),
  outExpo: (t) => (t >= 1 ? 1 : 1 - Math.pow(2, -10 * t)),
  inQuart: (t) => t * t * t * t,
  smoother: (t) => t * t * t * (t * (6 * t - 15) + 10),   // punchier than smoothstep
  whip: (t) => 1 - Math.pow(1 - t, 3.4),                  // camera: burst then settle
};

/** speed streak behind the dot: pos a few frames back → now */
function dotStreak(ctx, x0, y0, x1, y1, a) {
  const g = ctx.createLinearGradient(x0, y0, x1, y1);
  g.addColorStop(0, "rgba(53,208,165,0)");
  g.addColorStop(1, `rgba(53,208,165,${a})`);
  ctx.save();
  ctx.strokeStyle = g;
  ctx.lineWidth = 3.5;
  ctx.lineCap = "round";
  ctx.beginPath(); ctx.moveTo(x0, y0); ctx.lineTo(x1, y1); ctx.stroke();
  ctx.restore();
}

/* ------------------------------------------------------------------ assets */

const IMG = {};
const IMG_SOURCES = {
  roadmapDark: "../../shots/02-roadmap-dark-lighthouse.png",
  convLight: "../../shots/05-learning-conversation-light.png",
};

function loadImages() {
  return Promise.all(Object.entries(IMG_SOURCES).map(([k, src]) =>
    new Promise((resolve) => {
      const im = new Image();
      im.onload = () => { IMG[k] = im; resolve(); };
      im.onerror = () => { console.warn("missing", src); resolve(); };
      im.src = src;
    })));
}

/* ---------------------------------------------------------------- textures */

let fogTex = null, grainTiles = [];

function makeFogTex() {
  const s = 512;
  const cv = document.createElement("canvas");
  cv.width = s; cv.height = s;
  const g = cv.getContext("2d");
  const im = g.createImageData(s, s);
  for (let y = 0; y < s; y++) for (let x = 0; x < s; x++) {
    const u = x / s, v = y / s;
    let n = 0, amp = 0.55, f = 3;
    for (let o = 0; o < 5; o++) {
      n += amp * vnoise((Math.cos(u * Math.PI * 2) * 0.5 + 0.5) * f * 3 + o * 13.7,
                        (Math.sin(v * Math.PI * 2) * 0.5 + 0.5) * f * 3 + o * 7.1);
      amp *= 0.5; f *= 2.1;
    }
    n = Math.pow(clamp01(n), 1.6);
    const i = (y * s + x) * 4;
    im.data[i] = im.data[i + 1] = im.data[i + 2] = 255;
    im.data[i + 3] = n * 255;
  }
  g.putImageData(im, 0, 0);
  return cv;
}

function makeGrainTile(seed) {
  const s = 360;
  const cv = document.createElement("canvas");
  cv.width = s; cv.height = s;
  const g = cv.getContext("2d");
  const im = g.createImageData(s, s);
  const rng = mulberry32(seed);
  for (let i = 0; i < s * s; i++) {
    const v = 118 + (rng() - 0.5) * 90;
    im.data[i * 4] = im.data[i * 4 + 1] = im.data[i * 4 + 2] = v;
    im.data[i * 4 + 3] = 26;
  }
  g.putImageData(im, 0, 0);
  return cv;
}

function drawFog(ctx, t, mode, a) {
  ctx.save();
  ctx.globalCompositeOperation = mode === "screen" ? "screen" : "multiply";
  try { ctx.filter = "blur(26px)"; } catch (e) {}
  const layers = [
    { sc: 2.9, vx: 42, vy: 10, a: 0.5 },
    { sc: 2.0, vx: -29, vy: 16, a: 0.36 },
    { sc: 4.0, vx: 16, vy: -13, a: 0.44 },
  ];
  for (const Lyr of layers) {
    ctx.globalAlpha = a * Lyr.a;
    const size = 512 * Lyr.sc;
    const ox = ((t * Lyr.vx) % size + size) % size;
    const oy = ((t * Lyr.vy) % size + size) % size;
    for (let x = -1; x <= 1; x++) for (let y = -1; y <= 1; y++)
      ctx.drawImage(fogTex, x * size - ox + W / 2 - size / 2, y * size - oy + H / 2 - size / 2, size, size);
  }
  ctx.restore();
}

function drawGrain(ctx, t, s) {
  const tile = grainTiles[Math.floor(t * FPS) % grainTiles.length];
  ctx.save();
  ctx.globalAlpha = s ?? 0.5;
  ctx.globalCompositeOperation = "overlay";
  ctx.fillStyle = ctx.createPattern(tile, "repeat");
  ctx.fillRect(0, 0, W, H);
  ctx.restore();
}

function vignette(ctx, a) {
  const g = ctx.createRadialGradient(W / 2, H / 2, H * 0.36, W / 2, H / 2, H * 0.95);
  g.addColorStop(0, "rgba(0,0,0,0)");
  g.addColorStop(1, `rgba(0,0,0,${a})`);
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, W, H);
}

/* ----------------------------------------------------------------- helpers */

function alpha(ctx, a, fn) { ctx.save(); ctx.globalAlpha = clamp01(a); fn(); ctx.restore(); }

function monoTag(ctx, text, x, y, color, px, align) {
  ctx.save();
  ctx.font = F.mono(px || 16, 500);
  try { ctx.letterSpacing = "3px"; } catch (e) {}
  ctx.fillStyle = color || C.fogGrey;
  ctx.textAlign = align || "left";
  ctx.fillText(text, x, y);
  ctx.restore();
}

function signalDot(ctx, x, y, r, glow) {
  const g = ctx.createRadialGradient(x, y, 0, x, y, r * (glow ? 6 : 3));
  g.addColorStop(0, C.signal);
  g.addColorStop(0.25, "rgba(53,208,165,0.5)");
  g.addColorStop(1, "rgba(53,208,165,0)");
  ctx.fillStyle = g;
  ctx.beginPath(); ctx.arc(x, y, r * (glow ? 6 : 3), 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = "#eafff7";
  ctx.beginPath(); ctx.arc(x, y, r, 0, Math.PI * 2); ctx.fill();
}

function amberDot(ctx, x, y, r) {
  const g = ctx.createRadialGradient(x, y, 0, x, y, r * 5);
  g.addColorStop(0, "rgba(230,192,132,0.9)");
  g.addColorStop(0.3, "rgba(230,192,132,0.35)");
  g.addColorStop(1, "rgba(230,192,132,0)");
  ctx.fillStyle = g;
  ctx.beginPath(); ctx.arc(x, y, r * 5, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = "#f7e3bd";
  ctx.beginPath(); ctx.arc(x, y, r, 0, Math.PI * 2); ctx.fill();
}

function drawShot(ctx, img, cam) {
  if (!img) return;
  const cover = Math.max(W / img.width, H / img.height) * (cam.scale || 1);
  const cx = (cam.cx ?? 0.5) * img.width, cy = (cam.cy ?? 0.5) * img.height;
  ctx.drawImage(img, W / 2 - cx * cover + (cam.ox || 0), H / 2 - cy * cover + (cam.oy || 0), img.width * cover, img.height * cover);
}

function shotPoint(img, cam, fx, fy) {
  const cover = Math.max(W / img.width, H / img.height) * (cam.scale || 1);
  return {
    x: W / 2 + (fx - (cam.cx ?? 0.5)) * img.width * cover + (cam.ox || 0),
    y: H / 2 + (fy - (cam.cy ?? 0.5)) * img.height * cover + (cam.oy || 0),
    cover,
  };
}

/** paper-lip occlusion wipe: incoming image revealed by a sweeping slab */
function wipeShot(ctx, img, progress, angle) {
  if (!img) return;
  const p = EASE.inOutCubic(clamp01(progress));
  const dir = angle ?? 0.06;
  const cx = W / 2, cy = H / 2;
  const len = (W + H) * 1.2;
  const nx = Math.cos(dir), ny = Math.sin(dir);
  const ex = -ny, ey = nx;
  const d = (p * 2 - 1) * (len / 2 + 200);
  ctx.save();
  ctx.beginPath();
  ctx.moveTo(cx + nx * d - ex * len, cy + ny * d - ey * len);
  ctx.lineTo(cx + nx * d + ex * len, cy + ny * d + ey * len);
  ctx.lineTo(cx + nx * (d - len) + ex * len, cy + ny * (d - len) + ey * len);
  ctx.lineTo(cx + nx * (d - len) - ex * len, cy + ny * (d - len) - ey * len);
  ctx.closePath(); ctx.clip();
  drawShot(ctx, img, { cx: 0.55, cy: 0.20, scale: 1.7 });
  ctx.restore();
  ctx.save();
  ctx.translate(cx + nx * d, cy + ny * d);
  ctx.rotate(dir);
  const lip = ctx.createLinearGradient(-90, 0, 0, 0);
  lip.addColorStop(0, "rgba(246,242,234,0)");
  lip.addColorStop(1, "rgba(246,242,234,0.85)");
  ctx.fillStyle = lip;
  ctx.fillRect(-90, -H, 90, H * 2);
  ctx.restore();
}

/* -------------------------------------------- the world: nodes, path, pile */

const NODES = [
  { x: 0.16, y: 0.72, zh: "射影平面基础", en: "PROJECTIVE PLANE", state: "done" },
  { x: 0.28, y: 0.52, zh: "齐次坐标", en: "HOMOGENEOUS", state: "done" },
  { x: 0.42, y: 0.60, zh: "交比与调和分割", en: "CROSS RATIO", state: "current" },
  { x: 0.55, y: 0.44, zh: "对偶原理", en: "DUALITY", state: "pending" },
  { x: 0.66, y: 0.58, zh: "二次曲线", en: "CONICS", state: "pending" },
  { x: 0.77, y: 0.40, zh: "配极变换", en: "POLARITY", state: "pending" },
];
const CURRENT = 2;

/* extension beyond the last node, drawn in EXPAND */
const NODES_EXT = [{ x: 0.875, y: 0.30 }, { x: 0.96, y: 0.42 }, { x: 1.05, y: 0.30 }];

const PILE = [
  { x: 0.24, y: 0.40, s: 0.52, r: -0.045 },
  { x: 0.50, y: 0.35, s: 0.54, r: 0.028 },
  { x: 0.76, y: 0.40, s: 0.52, r: 0.05 },
  { x: 0.31, y: 0.67, s: 0.47, r: 0.035 },
  { x: 0.56, y: 0.65, s: 0.50, r: -0.028 },
  { x: 0.80, y: 0.67, s: 0.45, r: 0.045 },
];

let cards = null;      // SPEC2.buildAll()
let mapPath = null;    // catmull-rom sampled polyline with cumulative length
let mapPathExt = null;

function buildPath(nodeList) {
  const P = nodeList.map((n) => ({ x: n.x * W, y: n.y * H }));
  const pts = [];
  for (let i = 0; i < P.length - 1; i++) {
    const p0 = P[Math.max(0, i - 1)], p1 = P[i], p2 = P[i + 1], p3 = P[Math.min(P.length - 1, i + 2)];
    for (let u = 0; u <= 24; u++) {
      const t2 = u / 24, tt = t2 * t2, ttt = tt * t2;
      pts.push({
        x: 0.5 * (2 * p1.x + (-p0.x + p2.x) * t2 + (2 * p0.x - 5 * p1.x + 4 * p2.x - p3.x) * tt + (-p0.x + 3 * p1.x - 3 * p2.x + p3.x) * ttt),
        y: 0.5 * (2 * p1.y + (-p0.y + p2.y) * t2 + (2 * p0.y - 5 * p1.y + 4 * p2.y - p3.y) * tt + (-p0.y + 3 * p1.y - 3 * p2.y + p3.y) * ttt),
      });
    }
  }
  let L = 0;
  pts.forEach((p, i) => { if (i) L += Math.hypot(p.x - pts[i - 1].x, p.y - pts[i - 1].y); p.d = L; });
  return { pts, total: L };
}

function pathPoint(pathData, f) {
  const target = clamp01(f) * pathData.total;
  const pts = pathData.pts;
  let i = 0;
  while (i < pts.length - 1 && pts[i].d < target) i++;
  return pts[Math.max(0, i - 1)] || pts[0];
}

function drawPartial(ctx, pathData, f, style, width, dash) {
  const target = clamp01(f) * pathData.total;
  ctx.save();
  ctx.strokeStyle = style; ctx.lineWidth = width;
  if (dash) ctx.setLineDash(dash);
  ctx.beginPath();
  let started = false;
  for (const p of pathData.pts) {
    if (p.d > target) break;
    if (!started) { ctx.moveTo(p.x, p.y); started = true; } else ctx.lineTo(p.x, p.y);
  }
  ctx.stroke();
  ctx.restore();
}

function chartGrid(ctx, a) {
  alpha(ctx, a, () => {
    ctx.strokeStyle = "rgba(154,165,168,0.10)";
    ctx.lineWidth = 1;
    for (let x = 0; x <= W; x += 120) { ctx.beginPath(); ctx.moveTo(x + 0.5, 0); ctx.lineTo(x + 0.5, H); ctx.stroke(); }
    for (let y = 0; y <= H; y += 120) { ctx.beginPath(); ctx.moveTo(0, y + 0.5); ctx.lineTo(W, y + 0.5); ctx.stroke(); }
    ctx.strokeStyle = "rgba(154,165,168,0.06)";
    for (let i = 0; i < 5; i++) { ctx.beginPath(); ctx.ellipse(W / 2, H / 2, 400 + i * 220, 240 + i * 130, 0, 0, Math.PI * 2); ctx.stroke(); }
  });
}

function nodeLabel(ctx, n, a) {
  alpha(ctx, a, () => {
    ctx.font = F.mono(14, 500);
    try { ctx.letterSpacing = "2px"; } catch (e) {}
    ctx.fillStyle = "rgba(188,205,214,0.85)";
    ctx.fillText(n.en, n.x * W + 15, n.y * H - 10);
    ctx.font = F.song(16, 400);
    ctx.fillStyle = "rgba(246,242,234,0.8)";
    ctx.fillText(n.zh, n.x * W + 15, n.y * H + 12);
  });
}

function pileCardPos(i, t) {
  const P = PILE[i];
  return {
    x: P.x * W + Math.sin(t * 0.5 + i * 2.2) * 9,
    y: P.y * H + Math.cos(t * 0.42 + i * 1.7) * 7,
    s: P.s, r: P.r,
  };
}

function kwScreenPos(i, t) {
  const P = pileCardPos(i, t);
  const card = cards[i];
  const kx = (card.kw.x0 + card.kw.x1) / 2, ky = (card.kw.y0 + card.kw.y1) / 2;
  return {
    x: P.x + (kx - 0.5) * card.cv.width * P.s,
    y: P.y + (ky - 0.5) * card.cv.height * P.s,
  };
}

function drawPileCard(ctx, i, t, alphaMul) {
  const P = pileCardPos(i, t);
  const { cv } = cards[i];
  ctx.save();
  ctx.translate(P.x, P.y);
  ctx.rotate(P.r);
  ctx.scale(P.s, P.s);
  ctx.globalAlpha = alphaMul;
  ctx.shadowColor = "rgba(0,0,0,0.55)";
  ctx.shadowBlur = 34;
  ctx.shadowOffsetY = 16;
  ctx.drawImage(cv, -cv.width / 2, -cv.height / 2);
  ctx.restore();
}

/* ============================================================ S1 QUESTION */

const VP = { x: W * 0.5, y: H * 0.42 };

/* shared with S2: the pulse ring formula (continuous across the cut) */
function pulseRingR(t) {
  return seg(t, 3.4, 5.6, EASE.outQuad) * 1500;
}

function s1(ctx, t) {
  ctx.fillStyle = C.dark0;
  ctx.fillRect(0, 0, W, H);
  drawFog(ctx, t, "screen", 0.14);

  // the perspective study draws itself
  ctx.strokeStyle = "rgba(214,208,196,0.85)";
  // horizon
  const hp = seg(t, 0.6, 1.3, EASE.inOutCubic);
  if (hp > 0) {
    ctx.save();
    ctx.lineWidth = 1.6;
    ctx.beginPath();
    ctx.moveTo(W * 0.14, VP.y);
    ctx.lineTo(lerp(W * 0.14, W * 0.86, hp), VP.y);
    ctx.stroke();
    ctx.restore();
  }
  // rails converge
  const rp = seg(t, 1.2, 2.2, EASE.inOutCubic);
  if (rp > 0) {
    ctx.save();
    ctx.lineWidth = 2;
    for (const sx of [W * 0.26, W * 0.74]) {
      ctx.beginPath();
      ctx.moveTo(sx, H * 0.92);
      ctx.lineTo(lerp(sx, VP.x, rp), lerp(H * 0.92, VP.y, rp));
      ctx.stroke();
    }
    ctx.lineWidth = 1.2;
    const sn = Math.floor(rp * 11);
    for (let i = 1; i <= sn; i++) {
      const tt = Math.pow(i / 11, 1.9);
      const y = lerp(H * 0.90, VP.y, tt);
      const xl = lerp(W * 0.26, VP.x, tt), xr = lerp(W * 0.74, VP.x, tt);
      ctx.beginPath(); ctx.moveTo(xl, y); ctx.lineTo(xr, y); ctx.stroke();
    }
    ctx.restore();
  }
  // the dot is born at the vanishing point
  if (t > 2.15) {
    const bp = seg(t, 2.15, 2.45, E.outQuint);
    const breathe = 0.75 + 0.25 * Math.sin(t * 2.6);
    alpha(ctx, bp * breathe, () => signalDot(ctx, VP.x, VP.y, 5, true));
    const ring = seg(t, 2.15, 3.1, EASE.outCubic);
    if (ring < 1) alpha(ctx, (1 - ring) * 0.6, () => {
      ctx.strokeStyle = C.signal; ctx.lineWidth = 1.5;
      ctx.beginPath(); ctx.arc(VP.x, VP.y, 8 + ring * 240, 0, Math.PI * 2); ctx.stroke();
    });
  }
  if (!NO_TEXT && t > 2.5) {
    alpha(ctx, seg(t, 2.5, 2.9) * (1 - seg(t, 3.5, 3.9)), () => {
      const band = ctx.createLinearGradient(0, H * 0.70, 0, H * 0.88);
      band.addColorStop(0, "rgba(6,8,10,0)");
      band.addColorStop(0.5, "rgba(6,8,10,0.72)");
      band.addColorStop(1, "rgba(6,8,10,0)");
      ctx.fillStyle = band;
      ctx.fillRect(0, H * 0.70, W, H * 0.18);
      ctx.textAlign = "center";
      ctx.font = F.song(34, 400);
      ctx.fillStyle = "rgba(246,242,234,0.92)";
      ctx.fillText("平行的铁轨，为什么在远方相交？", W / 2, H * 0.78);
      monoTag(ctx, "A QUESTION", W / 2, H * 0.78 + 40, "rgba(53,208,165,0.75)", 14, "center");
      ctx.textAlign = "left";
    });
  }
  // the pulse begins before the scene hands off — motion crosses the cut
  if (t > 3.4) {
    const R = pulseRingR(t);
    alpha(ctx, 0.85, () => {
      ctx.strokeStyle = C.signal;
      ctx.lineWidth = 2;
      ctx.beginPath(); ctx.arc(VP.x, VP.y, R, 0, Math.PI * 2); ctx.stroke();
    });
  }
}

/* ======================================================== S2 TOO MUCH INFO */

function s2(ctx, t) {
  ctx.fillStyle = C.chart;
  ctx.fillRect(0, 0, W, H);
  drawFog(ctx, t, "screen", 0.12);

  // pulse ring (same formula as S1's tail) reveals papers as it passes
  const ringR = pulseRingR(t);
  const reveal = (i) => {
    const P = PILE[i];
    const d = Math.hypot(P.x * W - VP.x, P.y * H - VP.y);
    return seg(ringR - d, 0, 260, E.outQuint);
  };
  for (let i = 0; i < 6; i++) {
    const rv = reveal(i);
    if (rv > 0) drawPileCard(ctx, i, t, rv * 0.97);
  }
  if (t < 6.2) {
    alpha(ctx, 0.85 * (1 - seg(t, 5.4, 6.2)), () => {
      ctx.strokeStyle = C.signal;
      ctx.lineWidth = 2;
      ctx.beginPath(); ctx.arc(VP.x, VP.y, ringR, 0, Math.PI * 2); ctx.stroke();
      const g = ctx.createRadialGradient(VP.x, VP.y, Math.max(0, ringR - 90), VP.x, VP.y, ringR);
      g.addColorStop(0, "rgba(53,208,165,0)");
      g.addColorStop(1, "rgba(53,208,165,0.14)");
      ctx.fillStyle = g;
      ctx.beginPath(); ctx.arc(VP.x, VP.y, ringR, 0, Math.PI * 2); ctx.fill();
    });
  }

  // the dot never sits: it glides from the VP toward the first card
  const gx = lerp(VP.x, pileCardPos(0, t).x, seg(t, 6.2, 8.9, EASE.inOutCubic));
  const gy = lerp(VP.y, pileCardPos(0, t).y, seg(t, 6.2, 8.9, EASE.inOutCubic));
  const breathe = 0.7 + 0.3 * Math.sin(t * 2.6);
  alpha(ctx, breathe * 0.9 * (1 - seg(t, 8.9, 9.5)), () => signalDot(ctx, gx, gy, 4, true));

  monoTag(ctx, "SIX SOURCES · ONE QUESTION · NO ROUTE", W / 2, H * 0.07, "rgba(154,165,168,0.6)", 15, "center");

  // scan line sweeps the pile — and hands off into S3's first ring
  const scan = seg(t, 8.9, 9.6, E.whip);
  if (scan > 0) {
    const bx = lerp(-80, W + 80, scan);
    const bg = ctx.createLinearGradient(bx - 220, 0, bx + 6, 0);
    bg.addColorStop(0, "rgba(53,208,165,0)");
    bg.addColorStop(0.85, "rgba(53,208,165,0.10)");
    bg.addColorStop(1, "rgba(53,208,165,0.35)");
    ctx.fillStyle = bg;
    ctx.fillRect(bx - 220, 0, 226, H);
    ctx.fillStyle = "rgba(234,255,247,0.6)";
    ctx.fillRect(bx - 1.5, 0, 1.5, H);
  }
}

/* ========================================================== S3 STRUCTURE */

const EXTRACT_T0 = 9.7, EXTRACT_DT = 0.8;

function drawNodesRouteBase(ctx, t) {
  chartGrid(ctx, 1);
  NODES.forEach((n, i) => {
    const born = seg(t, EXTRACT_T0 + i * EXTRACT_DT + 0.45, EXTRACT_T0 + i * EXTRACT_DT + 0.78);
    if (born <= 0) return;
    alpha(ctx, born, () => {
      signalDot(ctx, n.x * W, n.y * H, n.state === "current" ? 5 : 3.5, n.state === "current");
      nodeLabel(ctx, n, 1);
    });
  });
}

function s3(ctx, t) {
  ctx.fillStyle = C.chart;
  ctx.fillRect(0, 0, W, H);
  drawFog(ctx, t, "screen", 0.10);

  // pile persists but dims as it is emptied
  for (let i = 0; i < 6; i++) {
    const drained = seg(t, EXTRACT_T0 + i * EXTRACT_DT + 0.3, EXTRACT_T0 + i * EXTRACT_DT + 0.75);
    drawPileCard(ctx, i, t, 0.97 - drained * 0.72);
  }

  drawNodesRouteBase(ctx, t);

  // the S2 scan line converges directly into the first extraction ring
  if (t < 9.95) {
    const conv = seg(t, 9.6, 9.95, E.outQuint);
    const kw0 = kwScreenPos(0, t);
    const bx = lerp(W + 80, kw0.x, conv);
    alpha(ctx, 1 - conv * 0.6, () => {
      ctx.fillStyle = "rgba(234,255,247,0.6)";
      ctx.fillRect(bx - 1.5, 0, 1.5, H * (1 - conv * 0.5));
    });
    if (conv > 0.4) {
      alpha(ctx, (conv - 0.4) / 0.6, () => {
        ctx.strokeStyle = C.signal;
        ctx.lineWidth = 1.8;
        ctx.beginPath(); ctx.ellipse(kw0.x, kw0.y, 160 * (1 - conv * 0.5), 60 * (1 - conv * 0.5), 0, 0, Math.PI * 2); ctx.stroke();
      });
    }
  }

  // extraction choreography per card — flights overlap the next ring
  for (let i = 0; i < 6; i++) {
    const t0 = EXTRACT_T0 + i * EXTRACT_DT;
    const local = t - t0;
    if (local < 0 || local > 1.15) continue;
    const kw = kwScreenPos(i, t);
    const n = NODES[i];
    const nx = n.x * W, ny = n.y * H;

    // 1) ring snaps onto the keyword
    const ringP = seg(local, 0, 0.18, E.outQuint);
    if (local < 0.45) {
      alpha(ctx, ringP * (1 - seg(local, 0.32, 0.45)), () => {
        ctx.strokeStyle = C.signal;
        ctx.lineWidth = 1.8;
        const R = lerp(120, 46, ringP);
        ctx.beginPath(); ctx.ellipse(kw.x, kw.y, R * 1.5, R * 0.5, 0, 0, Math.PI * 2); ctx.stroke();
      });
    }
    // 2) keyword glows green on the paper
    const glowP = seg(local, 0.14, 0.3);
    if (glowP > 0 && local < 0.7) {
      alpha(ctx, glowP * (1 - seg(local, 0.5, 0.7)), () => {
        ctx.font = F.song(19, 400);
        ctx.fillStyle = "#0d9a72";
        ctx.textAlign = "center";
        ctx.fillText(n.zh.length > 4 ? n.zh.slice(0, 4) : n.zh, kw.x - 22, kw.y + 4);
        ctx.textAlign = "left";
      });
    }
    // 3) the word rips off the paper and flies to its node, with a speed streak
    const flyP = seg(local, 0.28, 0.8, E.smoother);
    if (flyP > 0 && flyP < 1) {
      const mx = (kw.x + nx) / 2, my = Math.min(kw.y, ny) - 180;
      const at = (q) => ({
        x: (1 - q) * (1 - q) * kw.x + 2 * (1 - q) * q * mx + q * q * nx,
        y: (1 - q) * (1 - q) * kw.y + 2 * (1 - q) * q * my + q * q * ny,
      });
      const p0 = at(Math.max(0, flyP - 0.1)), p1 = at(flyP);
      dotStreak(ctx, p0.x, p0.y, p1.x, p1.y, 0.5);
      const sc = lerp(1, 0.12, flyP);
      alpha(ctx, Math.min(1, flyP * 2.4), () => {
        ctx.font = F.song(20 * Math.max(0.5, sc), 400);
        ctx.fillStyle = C.signal;
        ctx.textAlign = "center";
        ctx.fillText(n.zh, p1.x, p1.y);
        ctx.textAlign = "left";
      });
    }
  }

  // the reader lands on the first node before the route begins
  const land = seg(t, 15.0, 15.5, E.outQuint);
  if (land > 0) {
    const n0 = NODES[0];
    alpha(ctx, land, () => signalDot(ctx, n0.x * W, n0.y * H, 5, true));
  }

  monoTag(ctx, "EXTRACTION · TEXT → NODE", W / 2, H * 0.07, "rgba(154,165,168,0.6)", 15, "center");
}

/* ============================================================== S4 ROUTE */

function s4(ctx, t) {
  ctx.fillStyle = C.chart;
  ctx.fillRect(0, 0, W, H);
  drawFog(ctx, t, "screen", 0.10);

  // camera creeps from the start and accelerates into the current node
  const creep = seg(t, 16.2, 19.8, EASE.linear);
  const push = seg(t, 19.6, 20.4, E.inQuart);
  const cam = {
    cx: lerp(lerp(0.5, 0.475, creep), 0.45, push),
    cy: lerp(lerp(0.5, 0.55, creep), 0.585, push),
    scale: lerp(lerp(1, 1.08, creep), 1.3, push),
  };
  ctx.save();
  ctx.translate(W / 2, H / 2);
  ctx.scale(cam.scale, cam.scale);
  ctx.translate(-cam.cx * W, -cam.cy * H);

  chartGrid(ctx, 1);

  // the dot HOPS node to node: fast dart, brief dwell
  const HOPS = [15.8, 16.35, 16.95, 17.6, 18.3, 19.0];
  const travelF = (tt) => {
    if (tt <= HOPS[0]) return 0;
    for (let i = 0; i < HOPS.length - 1; i++) {
      if (tt < HOPS[i + 1]) {
        const hopDur = Math.min(0.32, (HOPS[i + 1] - HOPS[i]) * 0.62);
        return lerp(i / 5, (i + 1) / 5, E.outQuint(clamp01((tt - HOPS[i]) / hopDur)));
      }
    }
    return 1;
  };
  const drawn = travelF(t);
  drawPartial(ctx, mapPath, drawn, "rgba(53,208,165,0.75)", 2, [8, 7]);

  NODES.forEach((n, i) => {
    const nodeF = i / (NODES.length - 1);
    const reached = drawn >= nodeF - 0.01;
    const nx = n.x * W, ny = n.y * H;
    if (!reached) {
      alpha(ctx, 0.85, () => {
        ctx.strokeStyle = "rgba(154,165,168,0.5)";
        ctx.lineWidth = 1.4;
        ctx.beginPath(); ctx.arc(nx, ny, 5, 0, Math.PI * 2); ctx.stroke();
        nodeLabel(ctx, n, 0.45);
      });
      return;
    }
    if (n.state === "done") amberDot(ctx, nx, ny, 4);
    else if (n.state === "current") signalDot(ctx, nx, ny, 5.5, true);
    else {
      signalDot(ctx, nx, ny, 3.2, false);
    }
    nodeLabel(ctx, n, 1);
  });

  // dot position: hop forward, then dart back to CURRENT
  const backStart = 19.2;
  const dotF = (tt) => tt < backStart ? travelF(tt)
    : lerp(1, CURRENT / (NODES.length - 1), E.outQuint(seg(tt, backStart, 19.75)));
  const dpNow = pathPoint(mapPath, dotF(t));
  const dpPrev = pathPoint(mapPath, dotF(t - 0.05));
  const speed = Math.hypot(dpNow.x - dpPrev.x, dpNow.y - dpPrev.y);
  if (speed > 6) dotStreak(ctx, dpPrev.x, dpPrev.y, dpNow.x, dpNow.y, 0.55);
  signalDot(ctx, dpNow.x, dpNow.y, 5.5, true);

  // arrival pulse — the dot's landing ripples outward
  if (t > 19.75) {
    const ring = seg(t, 19.75, 20.5, EASE.outCubic);
    if (ring < 1) alpha(ctx, (1 - ring) * 0.55, () => {
      ctx.strokeStyle = C.signal; ctx.lineWidth = 1.6;
      ctx.beginPath(); ctx.arc(dpNow.x, dpNow.y, 8 + ring * 130, 0, Math.PI * 2); ctx.stroke();
    });
  }

  // the ring starts growing out of the arrived dot before the scene hands off
  if (t > 20.2) {
    const ex = seg(t, 20.2, 21.5, EASE.inCubic);
    const R = lerp(8, 1500, ex);
    ctx.save();
    ctx.strokeStyle = C.signal;
    ctx.lineWidth = Math.max(1.5, 6 - ex * 4);
    ctx.globalAlpha = 0.95;
    ctx.beginPath(); ctx.arc(dpNow.x, dpNow.y, R, 0, Math.PI * 2); ctx.stroke();
    ctx.restore();
  }
  ctx.restore();

  if (!NO_TEXT && t > 18.8) {
    alpha(ctx, seg(t, 18.8, 19.4) * (1 - seg(t, 20.0, 20.4)), () => {
      ctx.textAlign = "center";
      ctx.font = F.serif(46, true);
      ctx.fillStyle = "rgba(246,242,234,0.95)";
      ctx.fillText("Turn information into a journey.", W / 2, H * 0.88);
      ctx.textAlign = "left";
    });
  }
}

/* ======================================================= S5 ENTER PRODUCT */

const UI_CARD = { x0: 0.395, y0: 0.60, x1: 0.665, y1: 0.71 };  // roadmapDark current card

function s5(ctx, t) {
  // camera continuity from S4's final push
  const pushCam = { cx: 0.45, cy: 0.585, scale: 1.3 };
  const nodeScreen = {
    x: W / 2 + (NODES[CURRENT].x - pushCam.cx) * W * pushCam.scale,
    y: H / 2 + (NODES[CURRENT].y - pushCam.cy) * H * pushCam.scale,
  };
  const ex = seg(t, 20.2, 21.5, EASE.inCubic);   // shared with S4's tail

  if (t < 21.5) {
    // world still visible, the arrived dot expands into a ring
    ctx.fillStyle = C.chart;
    ctx.fillRect(0, 0, W, H);
    ctx.save();
    ctx.translate(W / 2, H / 2);
    ctx.scale(pushCam.scale, pushCam.scale);
    ctx.translate(-pushCam.cx * W, -pushCam.cy * H);
    chartGrid(ctx, 1 - ex);
    drawPartial(ctx, mapPath, 1, `rgba(53,208,165,${0.75 * (1 - ex)})`, 2, [8, 7]);
    NODES.forEach((n) => {
      alpha(ctx, 1 - ex, () => {
        if (n.state === "done") amberDot(ctx, n.x * W, n.y * H, 4);
        else signalDot(ctx, n.x * W, n.y * H, n.state === "current" ? 5 : 3.2, false);
        nodeLabel(ctx, n, 0.9);
      });
    });
    ctx.restore();
    const R = lerp(8, 1500, ex);
    drawFog(ctx, t * 1.2, "screen", ex * 0.85);
    ctx.save();
    ctx.strokeStyle = C.signal;
    ctx.lineWidth = Math.max(1.5, 6 - ex * 4);
    ctx.globalAlpha = 0.95;
    ctx.beginPath(); ctx.arc(nodeScreen.x, nodeScreen.y, R * pushCam.scale, 0, Math.PI * 2); ctx.stroke();
    ctx.restore();
    return;
  }

  // UI reveals inside the ring while the ring is still shrinking onto the card
  const rev = seg(t, 21.5, 22.7, E.outQuint);
  const camIn = { cx: 0.52, cy: 0.64, scale: 1.5 };
  const camOut = { cx: 0.485, cy: 0.55, scale: 1.06 };
  const pull = seg(t, 22.5, 23.4, E.whip);
  const cam = {
    cx: lerp(camIn.cx, camOut.cx, pull),
    cy: lerp(camIn.cy, camOut.cy, pull),
    scale: lerp(camIn.scale, camOut.scale, pull),
  };

  ctx.fillStyle = C.dark1;
  ctx.fillRect(0, 0, W, H);
  alpha(ctx, rev, () => drawShot(ctx, IMG.roadmapDark, cam));
  drawFog(ctx, t, "screen", (1 - rev) * 0.7 + 0.06);

  // ring morphs to the card highlight rect
  const cardP0 = shotPoint(IMG.roadmapDark, cam, UI_CARD.x0, UI_CARD.y0);
  const cardP1 = shotPoint(IMG.roadmapDark, cam, UI_CARD.x1, UI_CARD.y1);
  const morph = seg(t, 21.7, 22.9, E.whip);
  if (morph < 1) {
    const R = lerp(1500, Math.max(cardP1.x - cardP0.x, cardP1.y - cardP0.y) * 0.72, morph);
    alpha(ctx, 0.95 * (1 - morph * 0.4), () => {
      ctx.strokeStyle = C.signal;
      ctx.lineWidth = 2.5;
      ctx.beginPath(); ctx.arc(W / 2, H / 2, R, 0, Math.PI * 2); ctx.stroke();
    });
  }
  if (morph > 0.6) {
    alpha(ctx, seg(morph, 0.6, 1), () => {
      ctx.strokeStyle = C.signal;
      ctx.lineWidth = 1.6;
      ctx.strokeRect(cardP0.x, cardP0.y, cardP1.x - cardP0.x, cardP1.y - cardP0.y);
      monoTag(ctx, "CURRENT NODE · 交比与调和分割", cardP0.x, cardP0.y - 12, "rgba(53,208,165,0.9)", 15);
    });
  }

  // a small dot detaches and leads into the card — while the camera already moves
  const lead = seg(t, 23.2, 23.9, E.outQuint);
  if (lead > 0) {
    const dx = lerp(cardP0.x, W / 2, lead), dy = lerp(cardP0.y, H / 2, lead);
    signalDot(ctx, dx, dy, 4.5, true);
  }

  // zoom into the card — accelerating — then paper-wipe into the conversation
  const zoomIn = seg(t, 23.6, 24.3, E.inQuart);
  if (zoomIn > 0) {
    const cam2 = { cx: lerp(camOut.cx, 0.52, zoomIn), cy: lerp(camOut.cy, 0.655, zoomIn), scale: lerp(camOut.scale, 1.6, zoomIn) };
    drawShot(ctx, IMG.roadmapDark, cam2);
  }
  const wipe = seg(t, 24.3, 25.0, EASE.inOutCubic);
  if (wipe > 0) wipeShot(ctx, IMG.convLight, wipe, 0.05);
}

/* ============================================================ S6 LEARNING */

/* camera anchors in convLight image-fraction space */
const CAMS = {
  open: { cx: 0.55, cy: 0.20, scale: 1.7 },
  ans1: { cx: 0.70, cy: 0.30, scale: 2.0 },
  probe: { cx: 0.55, cy: 0.385, scale: 1.85 },
  ans2: { cx: 0.70, cy: 0.47, scale: 2.0 },
  conf: { cx: 0.55, cy: 0.565, scale: 1.7 },
};
/* green dot anchors (image fractions) */
const DOTS = {
  open: [0.60, 0.215], ans1: [0.775, 0.28], probe: [0.528, 0.349],
  ans2: [0.672, 0.467], conf: [0.585, 0.60],
};
const BEATS = [
  { until: 27.4, cam: CAMS.open, dot: DOTS.open },
  { until: 29.3, cam: CAMS.ans1, dot: DOTS.ans1 },
  { until: 31.5, cam: CAMS.probe, dot: DOTS.probe },
  { until: 33.3, cam: CAMS.ans2, dot: DOTS.ans2 },
  { until: 34.4, cam: CAMS.conf, dot: DOTS.conf },
];

function s6(ctx, t) {
  ctx.fillStyle = "#f2eee6";
  ctx.fillRect(0, 0, W, H);

  // current beat + blended camera; a slow drift keeps the frame breathing
  let bi = BEATS.findIndex((b) => t < b.until);
  if (bi < 0) bi = BEATS.length - 1;
  const prev = bi > 0 ? BEATS[bi - 1] : BEATS[0];
  const cur = BEATS[bi];
  const startOfBeat = bi > 0 ? BEATS[bi - 1].until : 25.0;
  const move = seg(t, startOfBeat, startOfBeat + 0.6, E.whip);
  const drift = { x: Math.sin(t * 0.5) * 0.004, y: Math.cos(t * 0.42) * 0.003 };
  const cam = {
    cx: lerp(prev.cam.cx, cur.cam.cx, move) + drift.x,
    cy: lerp(prev.cam.cy, cur.cam.cy, move) + drift.y,
    scale: lerp(prev.cam.scale, cur.cam.scale, move),
  };
  drawShot(ctx, IMG.convLight, cam);
  drawFog(ctx, t, "multiply", 0.05);

  // dot darts between emphasis anchors
  const dMove = seg(t, startOfBeat, startOfBeat + 0.3, E.outQuint);
  const dfx = lerp(prev.dot[0], cur.dot[0], dMove), dfy = lerp(prev.dot[1], cur.dot[1], dMove);
  const dp = shotPoint(IMG.convLight, cam, dfx, dfy);

  const onProbe = t >= 29.3 && t < 31.5;
  const onAns2 = t >= 31.5 && t < 33.3;

  if (onProbe) {
    // dot opens into the circling ellipse around 「保持不变」
    const pt = shotPoint(IMG.convLight, cam, DOTS.probe[0], DOTS.probe[1]);
    const scan = seg(t, 29.9, 31.0, EASE.inOutCubic);
    const R = 62 * (pt.cover / 1.1);
    ctx.save();
    ctx.strokeStyle = "#0d9a72";
    ctx.lineWidth = 2.4;
    ctx.beginPath();
    ctx.ellipse(pt.x, pt.y, R * 1.5, R * 0.5, -0.02, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * scan);
    ctx.stroke();
    ctx.restore();
    if (scan >= 1) monoTag(ctx, "KEYWORD · 保持不变", pt.x - R * 1.5, pt.y - R * 0.72, "#0d7a5c", 16);
    else signalDot(ctx, pt.x + Math.cos(-Math.PI / 2 + Math.PI * 2 * scan) * R * 1.5, pt.y + Math.sin(-Math.PI / 2 + Math.PI * 2 * scan) * R * 0.5, 4, false);
  } else if (onAns2) {
    // ellipse collapses into an underline under 「交比」
    const pt = shotPoint(IMG.convLight, cam, DOTS.ans2[0], DOTS.ans2[1]);
    const u = seg(t, 32.2, 33.1, E.outQuint);
    ctx.save();
    ctx.strokeStyle = "#0d9a72";
    ctx.lineWidth = 3;
    const w2 = 96 * pt.cover;
    ctx.beginPath();
    ctx.moveTo(pt.x - w2 * 0.4, pt.y);
    ctx.lineTo(pt.x - w2 * 0.4 + w2 * u, pt.y);
    ctx.stroke();
    ctx.restore();
    if (u >= 1) monoTag(ctx, "THE REAL INVARIANT", pt.x - w2 * 0.4, pt.y + 26, "#0d7a5c", 14);
    else signalDot(ctx, pt.x - w2 * 0.4 + w2 * u, pt.y - 10, 4, false);
  } else {
    signalDot(ctx, dp.x, dp.y, 4.5, true);
  }

  // realization pulse overlaps the handoff to S7
  if (t > 33.9) {
    const ring = seg(t, 33.9, 34.6, EASE.outCubic);
    if (ring < 1) alpha(ctx, (1 - ring) * 0.7, () => {
      ctx.strokeStyle = "#0d9a72";
      ctx.lineWidth = 2;
      ctx.beginPath(); ctx.arc(dp.x, dp.y, 10 + ring * 160, 0, Math.PI * 2); ctx.stroke();
    });
  }

  monoTag(ctx, "ONE QUESTION · GUIDED, NOT GIVEN", W / 2, H * 0.06, "rgba(138,133,120,0.7)", 14, "center");
}

/* ======================================================= S7 UNDERSTANDING */

function s7(ctx, t) {
  if (t < 36.9) {
    // dot rises out of the dimming conversation — lift starts before the pulse ends
    const lift = seg(t, 34.4, 35.5, EASE.inOutCubic);
    ctx.fillStyle = "#f2eee6";
    ctx.fillRect(0, 0, W, H);
    alpha(ctx, 1 - lift * 0.85, () => drawShot(ctx, IMG.convLight, { ...CAMS.conf, scale: lerp(1.7, 1.5, lift) }));
    // darken toward night
    ctx.fillStyle = `rgba(6,8,10,${lift * 0.85})`;
    ctx.fillRect(0, 0, W, H);
    const from = shotPoint(IMG.convLight, CAMS.conf, DOTS.conf[0], DOTS.conf[1]);
    const dx = lerp(from.x, W * 0.5, lift), dy = lerp(from.y, H * 0.44, lift);
    signalDot(ctx, dx, dy, lerp(4.5, 6, lift), true);

    // mastery manifests
    const m = seg(t, 35.3, 36.3, E.outQuint);
    if (m > 0) {
      alpha(ctx, m, () => {
        monoTag(ctx, "MASTERY +0.12", W * 0.5 + 26, H * 0.44 + 4, C.signal, 16);
        monoTag(ctx, "NODE LIT · 交比", W * 0.5 + 26, H * 0.44 + 28, "rgba(154,165,168,0.9)", 13);
        ctx.strokeStyle = "rgba(53,208,165,0.45)";
        ctx.setLineDash([4, 5]);
        ctx.beginPath(); ctx.moveTo(W * 0.5, H * 0.44 + 12);
        ctx.quadraticCurveTo(W * 0.52, H * 0.62, W * 0.55, H * 0.70);
        ctx.stroke(); ctx.setLineDash([]);
        ctx.strokeStyle = "rgba(154,165,168,0.4)";
        ctx.setLineDash([6, 6]);
        ctx.beginPath(); ctx.moveTo(W * 0.25, H * 0.72);
        ctx.quadraticCurveTo(W * 0.5, H * 0.62, W * 0.75, H * 0.71);
        ctx.stroke(); ctx.setLineDash([]);
      });
    }
    return;
  }
  // silence, then the thesis — short, then the world starts expanding over it
  if (t >= 38.2) s8(ctx, t);
  else { ctx.fillStyle = "#0a0c0e"; ctx.fillRect(0, 0, W, H); }
  ctx.textAlign = "center";
  alpha(ctx, seg(t, 37.0, 37.4) * (1 - seg(t, 38.3, 38.6)), () => {
    ctx.font = F.serif(60, true);
    ctx.fillStyle = "rgba(246,242,234,0.6)";
    ctx.fillText("Not answers.", W / 2, H * 0.47);
  });
  alpha(ctx, seg(t, 37.7, 38.1) * (1 - seg(t, 38.3, 38.6)), () => {
    ctx.font = F.serif(74, true);
    ctx.fillStyle = "#f6f2ea";
    ctx.fillText("Understanding.", W / 2, H * 0.60);
    ctx.strokeStyle = C.signal;
    ctx.lineWidth = 2;
    ctx.beginPath(); ctx.moveTo(W / 2 - 150, H * 0.60 + 26); ctx.lineTo(W / 2 + 150, H * 0.60 + 26); ctx.stroke();
  });
  ctx.textAlign = "left";
}

/* ============================================================= S8 EXPAND */

const SUBJECTS = [
  { name: "PHYSICS", x: 0.13, y: 0.30 },
  { name: "HISTORY", x: 0.24, y: 0.80 },
  { name: "BIOLOGY", x: 0.36, y: 0.16 },
  { name: "CODE", x: 0.62, y: 0.84 },
  { name: "PHILOSOPHY", x: 0.74, y: 0.18 },
  { name: "MUSIC THEORY", x: 0.88, y: 0.62 },
];

let ghostCards = null;

function s8(ctx, t) {
  ctx.fillStyle = C.chart;
  ctx.fillRect(0, 0, W, H);
  drawFog(ctx, t, "screen", 0.14);

  // the zoom-out releases immediately out of the thesis — no dead frame
  const zoom = seg(t, 38.2, 41.2, E.whip);
  const cam = { cx: lerp(0.45, 0.5, zoom), cy: lerp(0.585, 0.5, zoom), scale: lerp(1.3, 0.8, zoom) };
  ctx.save();
  ctx.translate(W / 2, H / 2);
  ctx.scale(cam.scale, cam.scale);
  ctx.translate(-cam.cx * W, -cam.cy * H);

  chartGrid(ctx, seg(t, 38.2, 39.6));
  drawPartial(ctx, mapPath, 1, "rgba(53,208,165,0.55)", 2, [8, 7]);
  // route keeps growing beyond the last node
  const ext = seg(t, 39.8, 41.8, EASE.inOutCubic);
  if (ext > 0) drawPartial(ctx, mapPathExt, ext, "rgba(53,208,165,0.4)", 1.6, [4, 8]);

  NODES.forEach((n) => {
    const nx = n.x * W, ny = n.y * H;
    if (n.state === "done") amberDot(ctx, nx, ny, 3.5);
    else if (n.state === "current") signalDot(ctx, nx, ny, 5.5, true);
    else signalDot(ctx, nx, ny, 3, false);
    nodeLabel(ctx, n, 0.85);
  });
  ctx.restore();

  // more worlds rise in quick succession
  const subj = seg(t, 41.0, 44.5);
  if (subj > 0) {
    SUBJECTS.forEach((s, i) => {
      const a = seg(t, 41.0 + i * 0.3, 41.45 + i * 0.3, E.outQuint);
      if (a <= 0) return;
      const rise = (1 - a) * 18;
      alpha(ctx, a * 0.95, () => {
        const x = s.x * W, y = s.y * H + rise;
        signalDot(ctx, x - 14, y - 6, 2.2, false);
        monoTag(ctx, s.name, x, y, "rgba(246,242,234,0.9)", 24);
        ctx.strokeStyle = "rgba(53,208,165,0.35)";
        ctx.setLineDash([4, 6]);
        ctx.beginPath(); ctx.moveTo(x - 14, y + 18);
        ctx.quadraticCurveTo(x + 40, y + 34, x + 96, y + 22);
        ctx.stroke(); ctx.setLineDash([]);
      });
    });
    // archival ghosts as the texture of those worlds
    alpha(ctx, subj * 0.13, () => {
      const ghosts = [[0.18, 0.42, 0.30], [0.44, 0.28, 0.27], [0.68, 0.45, 0.28]];
      ghosts.forEach(([gx, gy, gs], i) => {
        const cv = ghostCards[i];
        if (!cv) return;
        ctx.save();
        ctx.translate(gx * W, gy * H);
        ctx.scale(gs, gs);
        ctx.drawImage(cv, -cv.width / 2, -cv.height / 2);
        ctx.restore();
      });
    });
  }

  // distant beacon
  const bc = seg(t, 43.2, 44.0);
  if (bc > 0) alpha(ctx, bc, () => amberDot(ctx, W * 0.9, H * 0.24, 3.5));

  if (!NO_TEXT && t > 43.8) {
    alpha(ctx, seg(t, 43.8, 44.5) * (1 - seg(t, 45.4, 46.0)), () => {
      ctx.textAlign = "center";
      ctx.font = F.serif(46, true);
      ctx.fillStyle = "rgba(246,242,234,0.95)";
      ctx.fillText("Any subject can become a journey.", W / 2, H * 0.5);
      ctx.textAlign = "left";
    });
  }
}

/* ============================================================== S9 ORBIT */

const ELL = { cx: W * 0.5, cy: H * 0.47, rx: W * 0.24, ry: H * 0.15, tilt: -0.22 };
function ellPoint(a) {
  const x = Math.cos(a) * ELL.rx, y = Math.sin(a) * ELL.ry;
  return {
    x: ELL.cx + x * Math.cos(ELL.tilt) - y * Math.sin(ELL.tilt),
    y: ELL.cy + x * Math.sin(ELL.tilt) + y * Math.cos(ELL.tilt),
  };
}

function s9(ctx, t) {
  ctx.fillStyle = "#05070a";
  ctx.fillRect(0, 0, W, H);
  drawFog(ctx, t * 0.5, "screen", 0.08);

  // the route's end curls into the orbit ellipse — the dot launches fast, settles into the loop
  const join = { x: W * 0.735, y: H * 0.42 };          // where route was heading
  const e0 = ellPoint(0);                               // ellipse entry (amber junction)
  const travelB = seg(t, 46.7, 47.6, E.outQuint);       // dart along the joining curve
  const travelE = seg(t, 47.6, 51.1, (q) => 1 - Math.pow(1 - q, 2.6)); // fast sweep, gentle close

  // joining curve: bezier from join → e0
  const c1 = { x: join.x + 40, y: join.y - 130 }, c2 = { x: e0.x + 130, y: e0.y - 40 };
  const bez = (q) => ({
    x: (1 - q) ** 3 * join.x + 3 * (1 - q) ** 2 * q * c1.x + 3 * (1 - q) * q * q * c2.x + q ** 3 * e0.x,
    y: (1 - q) ** 3 * join.y + 3 * (1 - q) ** 2 * q * c1.y + 3 * (1 - q) * q * q * c2.y + q ** 3 * e0.y,
  });

  // draw the walked route fading, the new line bright
  const fadeOld = 1 - seg(t, 46.5, 48.5);
  alpha(ctx, fadeOld * 0.5, () => {
    ctx.strokeStyle = "rgba(53,208,165,0.5)";
    ctx.setLineDash([8, 7]);
    ctx.lineWidth = 1.6;
    ctx.beginPath();
    ctx.moveTo(W * 0.3, H * 0.62);
    ctx.quadraticCurveTo(W * 0.55, H * 0.52, join.x, join.y);
    ctx.stroke();
    ctx.setLineDash([]);
  });

  // draw joining curve progressively
  if (travelB > 0) {
    ctx.save();
    ctx.strokeStyle = C.signal;
    ctx.lineWidth = 2;
    ctx.beginPath();
    for (let q = 0; q <= travelB; q += 0.02) {
      const p = bez(q);
      q === 0 ? ctx.moveTo(p.x, p.y) : ctx.lineTo(p.x, p.y);
    }
    ctx.stroke();
    ctx.restore();
  }
  // ellipse progressively
  if (travelE > 0) {
    ctx.save();
    ctx.strokeStyle = C.signal;
    ctx.lineWidth = 2;
    ctx.beginPath();
    const aEnd = travelE * Math.PI * 2;
    for (let a = 0; a <= aEnd; a += 0.02) {
      const p = ellPoint(a);
      a === 0 ? ctx.moveTo(p.x, p.y) : ctx.lineTo(p.x, p.y);
    }
    ctx.stroke();
    ctx.restore();
  }

  // amber junction light
  const aj = seg(t, 47.4, 48.0);
  if (aj > 0) alpha(ctx, aj, () => amberDot(ctx, e0.x, e0.y, 4));

  // the dot rides the path, streaking while fast
  const dotAt = (tt) => {
    const b = seg(tt, 46.7, 47.6, E.outQuint);
    const e = seg(tt, 47.6, 51.1, (q) => 1 - Math.pow(1 - q, 2.6));
    if (b < 1) return bez(b);
    if (e < 1) return ellPoint(e * Math.PI * 2);
    return ellPoint(Math.PI * 2 + 0.15 * Math.sin((tt - 51.1) * 1.4));
  };
  const dp = dotAt(t);
  const dpPrev = dotAt(t - 0.055);
  if (Math.hypot(dp.x - dpPrev.x, dp.y - dpPrev.y) > 6) dotStreak(ctx, dpPrev.x, dpPrev.y, dp.x, dp.y, 0.6);
  signalDot(ctx, dp.x, dp.y, 5, true);

  // wordmark arrives as the loop closes — not after
  const wm = seg(t, 50.8, 51.8, E.outQuint);
  if (wm > 0) alpha(ctx, wm, () => {
    ctx.textAlign = "center";
    ctx.font = F.song(126, 400);
    ctx.fillStyle = "#f6f2ea";
    ctx.fillText("学 涯", W / 2, H * 0.485);
    ctx.font = F.mono(28, 500);
    try { ctx.letterSpacing = "14px"; } catch (e) {}
    ctx.fillStyle = "rgba(246,242,234,0.75)";
    ctx.fillText("ORBIT", W / 2 + 7, H * 0.485 + 62);
    try { ctx.letterSpacing = "0px"; } catch (e) {}
    ctx.textAlign = "left";
  });
  if (!NO_TEXT && t > 52.0) {
    alpha(ctx, seg(t, 52.0, 52.9), () => {
      ctx.textAlign = "center";
      ctx.font = F.serif(29, true);
      ctx.fillStyle = "rgba(246,242,234,0.8)";
      ctx.fillText("Find your way through knowledge.", W / 2, H * 0.72);
      ctx.textAlign = "left";
    });
  }
}

/* ================================================================ frame */

function renderFrameAbs(ctx, t) {
  ctx.clearRect(0, 0, W, H);
  if (t < 3.6) s1(ctx, t);
  else if (t < 9.6) s2(ctx, t);
  else if (t < 15.6) s3(ctx, t);
  else if (t < 20.4) s4(ctx, t);
  else if (t < 25.0) s5(ctx, t);
  else if (t < 34.4) s6(ctx, t);
  else if (t < 38.6) s7(ctx, t);
  else if (t < 46.0) s8(ctx, t);
  else s9(ctx, t);
  drawGrain(ctx, t, 0.5);
  vignette(ctx, 0.3);
}

window.__film = {
  ready: (async () => {
    fogTex = makeFogTex();
    grainTiles = [makeGrainTile(11), makeGrainTile(22), makeGrainTile(33), makeGrainTile(44)];
    cards = SPEC2.buildAll();
    ghostCards = [SPEC.moonChart(), SPEC.tideTable(), SPEC.birdWing()];
    mapPath = buildPath(NODES);
    mapPathExt = buildPath([NODES[NODES.length - 1], ...NODES_EXT]);
    await loadImages();
    await document.fonts.ready;
    return true;
  })(),
  render(t) {
    const ctx = document.getElementById("stage").getContext("2d");
    renderFrameAbs(ctx, t);
  },
  DUR, FPS,
};
