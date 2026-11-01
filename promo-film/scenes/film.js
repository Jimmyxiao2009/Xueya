/* ORBIT promo — film engine.
 * Single-canvas deterministic renderer: renderFrame(tSeconds) paints the frame.
 * Timeline (seconds):
 *   S1 LOST     0.0 – 5.5
 *   S2 ARCHIVE  5.5 – 13.2
 *   S3 MAP     13.2 – 21.0
 *   S4 ORBIT   21.0 – 32.0
 *   S5 THINK   32.0 – 42.0
 *   S6 WORLD   42.0 – 52.0
 *   S7 BRAND   52.0 – 60.0
 */
"use strict";

const W = 1920, H = 1080, FPS = 30, DUR = 60;

const C = {
  ink: "#1b1d20",
  charcoal: "#23262b",
  paper: "#f6f2ea",
  warmWhite: "#fffefa",
  fogGrey: "#9aa5a8",
  sage: "#8fa08c",
  sky: "#bccdd6",
  signal: "#35d0a5",
  signalDim: "rgba(53,208,165,0.55)",
  amber: "#e6c084",
  dark0: "#06080a",
  dark1: "#0d1114",
  chart: "#0b0e12",
};

const F = {
  serif: (px, italic) => `${italic ? "italic " : ""}400 ${px}px "Palatino Linotype","Book Antiqua",serif`,
  song: (px, weight) => `${weight || 400} ${px}px "SimSun","Songti SC","STSong",serif`,
  mono: (px, weight) => `${weight || 500} ${px}px "Cascadia Mono","Consolas",monospace`,
  sans: (px, weight) => `${weight || 400} ${px}px "Segoe UI","Microsoft YaHei",sans-serif`,
};

/* ---------------------------------------------------------------- assets */

const IMG = {};
const IMG_SOURCES = {
  roadmapDark: "../shots/02-roadmap-dark-lighthouse.png",
  lessonDark: "../shots/08-lesson-dark-lighthouse.png",
  convLight: "../shots/05-learning-conversation-light.png",
  roadmapLight: "../shots/01-roadmap-light-buoy.png",
  quizLight: "../shots/06-learning-quiz-light.png",
  generating: "../shots/11-generating-light.png",
};

function loadImages() {
  return Promise.all(
    Object.entries(IMG_SOURCES).map(([k, src]) =>
      new Promise((resolve) => {
        const im = new Image();
        im.onload = () => { IMG[k] = im; resolve(); };
        im.onerror = () => { console.warn("missing image", src); resolve(); };
        im.src = src;
      })
    )
  );
}

/* -------------------------------------------------------------- textures */

let fogTex = null, grainTiles = [];

function makeFogTex() {
  const s = 512;
  const cv = document.createElement("canvas");
  cv.width = s; cv.height = s;
  const g = cv.getContext("2d");
  const im = g.createImageData(s, s);
  for (let y = 0; y < s; y++) {
    for (let x = 0; x < s; x++) {
      // tileable-ish fbm
      const u = x / s, v = y / s;
      let n = 0, amp = 0.55, f = 3;
      for (let o = 0; o < 5; o++) {
        n += amp * vnoise(
          (Math.cos(u * Math.PI * 2) * 0.5 + 0.5) * f * 3 + o * 13.7,
          (Math.sin(v * Math.PI * 2) * 0.5 + 0.5) * f * 3 + o * 7.1
        );
        amp *= 0.5; f *= 2.1;
      }
      n = Math.pow(clamp01(n), 1.6);
      const i = (y * s + x) * 4;
      im.data[i] = im.data[i + 1] = im.data[i + 2] = 255;
      im.data[i + 3] = n * 255;
    }
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

/** drifting fog; mode "screen" for dark scenes, "multiply" for paper scenes */
function drawFog(ctx, t, mode, alpha) {
  ctx.save();
  ctx.globalCompositeOperation = mode === "screen" ? "screen" : "multiply";
  try { ctx.filter = "blur(26px)"; } catch (e) {}
  const layers = [
    { sc: 2.9, vx: 26, vy: 6, a: 0.5 },
    { sc: 2.0, vx: -18, vy: 10, a: 0.36 },
    { sc: 4.0, vx: 10, vy: -8, a: 0.44 },
  ];
  for (const L of layers) {
    ctx.globalAlpha = alpha * L.a;
    const size = 512 * L.sc;
    const ox = ((t * L.vx) % size + size) % size;
    const oy = ((t * L.vy) % size + size) % size;
    for (let x = -1; x <= 1; x++)
      for (let y = -1; y <= 1; y++)
        ctx.drawImage(fogTex, x * size - ox + W / 2 - size / 2, y * size - oy + H / 2 - size / 2, size, size);
  }
  ctx.restore();
}

function drawGrain(ctx, t, strength) {
  const tile = grainTiles[Math.floor(t * FPS) % grainTiles.length];
  ctx.save();
  ctx.globalAlpha = strength ?? 0.55;
  ctx.globalCompositeOperation = "overlay";
  const pat = ctx.createPattern(tile, "repeat");
  ctx.fillStyle = pat;
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

/* ------------------------------------------------------------- helpers */

function alpha(ctx, a, fn) {
  ctx.save();
  ctx.globalAlpha = clamp01(a);
  fn();
  ctx.restore();
}

function monoTag(ctx, text, x, y, color, px, align) {
  ctx.save();
  ctx.font = F.mono(px || 17, 500);
  ctx.fillStyle = color || C.fogGrey;
  ctx.textAlign = align || "left";
  try { ctx.letterSpacing = "3px"; } catch (e) {}
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

function contourRing(ctx, x, y, r, seed, color, wob) {
  ctx.save();
  ctx.strokeStyle = color;
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (let a = 0; a <= Math.PI * 2 + 0.01; a += 0.12) {
    const rr = r * (1 + (wob ?? 0.18) * fbm(Math.cos(a) * 1.5 + seed, Math.sin(a) * 1.5 + seed * 2, 3));
    const px = x + Math.cos(a) * rr, py = y + Math.sin(a) * rr;
    a === 0 ? ctx.moveTo(px, py) : ctx.lineTo(px, py);
  }
  ctx.closePath(); ctx.stroke();
  ctx.restore();
}

/** draw image full-frame cover with camera {cx,cy,scale} in normalized image space */
function drawShot(ctx, img, cam) {
  if (!img) return;
  const iw = img.width, ih = img.height;
  const cover = Math.max(W / iw, H / ih) * (cam.scale || 1);
  const dw = iw * cover, dh = ih * cover;
  const cx = (cam.cx ?? 0.5) * iw, cy = (cam.cy ?? 0.5) * ih;
  const dx = W / 2 - cx * cover + (cam.ox || 0), dy = H / 2 - cy * cover + (cam.oy || 0);
  ctx.drawImage(img, dx, dy, dw, dh);
}

/** screen-space position of an image-space point under a drawShot camera */
function shotPoint(img, cam, fx, fy) {
  const cover = Math.max(W / img.width, H / img.height) * (cam.scale || 1);
  return {
    x: W / 2 + (fx - (cam.cx ?? 0.5)) * img.width * cover + (cam.ox || 0),
    y: H / 2 + (fy - (cam.cy ?? 0.5)) * img.height * cover + (cam.oy || 0),
    cover,
  };
}

/** paper-card occlusion wipe: incoming image revealed by a sweeping rotated slab */
function wipeShot(ctx, img, progress, angle) {
  if (!img) return;
  const p = EASE.inOutCubic(clamp01(progress));
  const dir = angle ?? 0.06;
  ctx.save();
  ctx.beginPath();
  const cx = W / 2, cy = H / 2;
  const len = (W + H) * 1.2;
  const nx = Math.cos(dir), ny = Math.sin(dir);
  const ex = -ny, ey = nx;
  const d = (p * 2 - 1) * (len / 2 + 200);
  ctx.moveTo(cx + nx * d - ex * len, cy + ny * d - ey * len);
  ctx.lineTo(cx + nx * d + ex * len, cy + ny * d + ey * len);
  ctx.lineTo(cx + nx * (d - len) + ex * len, cy + ny * (d - len) + ey * len);
  ctx.lineTo(cx + nx * (d - len) - ex * len, cy + ny * (d - len) - ey * len);
  ctx.closePath();
  ctx.clip();
  drawShot(ctx, img, { cx: 0.5, cy: 0.5, scale: 1.02 });
  ctx.restore();
  // slab edge: a paper lip
  ctx.save();
  ctx.translate(cx + nx * d, cy + ny * d);
  ctx.rotate(dir);
  const lipg = ctx.createLinearGradient(-90, 0, 0, 0);
  lipg.addColorStop(0, "rgba(246,242,234,0)");
  lipg.addColorStop(1, "rgba(246,242,234,0.85)");
  ctx.fillStyle = lipg;
  ctx.fillRect(-90, -H, 90, H * 2);
  ctx.restore();
}

/* ============================================================ S1 · LOST */

const SIG = { x: W * 0.62, y: H * 0.44 };

function s1(ctx, t) {
  ctx.fillStyle = C.dark0;
  ctx.fillRect(0, 0, W, H);
  drawFog(ctx, t, "screen", 0.16 + 0.05 * Math.sin(t * 0.7));

  // faint world-instrument traces around the signal
  const wake = seg(t, 2.2, 3.9, EASE.outCubic);
  alpha(ctx, wake * 0.5, () => {
    monoTag(ctx, "39°54′N  116°23′E", SIG.x + 46, SIG.y - 34, "rgba(154,165,168,0.8)", 16);
    monoTag(ctx, "DEPTH —", SIG.x + 46, SIG.y - 12, "rgba(154,165,168,0.5)", 14);
    monoTag(ctx, "(AB,CD) = −1", SIG.x - 210, SIG.y + 96, "rgba(154,165,168,0.55)", 15);
    monoTag(ctx, "FIELD NOTE 001", SIG.x - 260, SIG.y - 120, "rgba(154,165,168,0.4)", 13);
    contourRing(ctx, SIG.x, SIG.y, 120, 3.1, "rgba(154,165,168,0.30)", 0.22);
    contourRing(ctx, SIG.x, SIG.y, 200, 7.7, "rgba(154,165,168,0.18)", 0.3);
  });

  // the signal
  const ping = seg(t, 1.15, 1.5, EASE.outCubic);
  const breathe = 0.72 + 0.28 * Math.sin(t * 2.4);
  if (t > 1.15) {
    alpha(ctx, breathe, () => signalDot(ctx, SIG.x, SIG.y, 5, true));
    // ping ring once
    const ring = seg(t, 1.15, 2.3, EASE.outCubic);
    if (ring < 1) {
      alpha(ctx, (1 - ring) * 0.7, () => {
        ctx.strokeStyle = C.signal;
        ctx.lineWidth = 1.5;
        ctx.beginPath(); ctx.arc(SIG.x, SIG.y, 8 + ring * 260, 0, Math.PI * 2); ctx.stroke();
      });
    }
    if (ping < 1) alpha(ctx, ping, () => signalDot(ctx, SIG.x, SIG.y, 5 + (1 - ping) * 10, false));
  }

  // copy — two lines, hard cut between
  ctx.textAlign = "center";
  if (t >= 3.35 && t < 4.45) {
    alpha(ctx, Math.min(seg(t, 3.35, 3.8), 1 - seg(t, 4.2, 4.45)), () => {
      ctx.font = F.serif(46, true);
      ctx.fillStyle = "rgba(246,242,234,0.92)";
      ctx.fillText("You know what you want to learn.", W / 2, H * 0.52);
    });
  }
  if (t >= 4.65) {
    alpha(ctx, seg(t, 4.65, 5.05), () => {
      ctx.font = F.serif(46, true);
      ctx.fillStyle = "rgba(246,242,234,0.92)";
      ctx.fillText("You just don’t know the route.", W / 2, H * 0.52);
    });
  }
  ctx.textAlign = "left";
}

/* ========================================================= S2 · ARCHIVE */

const CARDS = [
  { make: "moonChart", stamp: "SPECIMEN 01 · LUNA", w: 620 },
  { make: "tideTable", stamp: "SPECIMEN 02 · TIDE", w: 620 },
  { make: "birdWing", stamp: "SPECIMEN 03 · WING", w: 620 },
  { make: "geoProof", stamp: "SPECIMEN 04 · PROOF", w: 620 },
  { make: "pdfPage", stamp: "SPECIMEN 05 · PAGE", w: 560 },
  { make: "contourMap", stamp: "SPECIMEN 06 · RELIEF", w: 700 },
  { make: "codeFragment", stamp: "SPECIMEN 07 · CODE", w: 700 },
];
let cardCanvas = null;

function buildCards() {
  cardCanvas = CARDS.map((c) => {
    const cv = SPEC[c.make]();
    return { cv, stamp: c.stamp };
  });
}

/* card timeline: each shows ~0.72s with 0.22s wipe overlap */
const CARD_T0 = 5.55, CARD_DT = 0.72, CARD_IN = 0.24;
const FREEZE_T = 10.9, SCAN_T0 = 11.15, SCAN_T1 = 13.0;

function s2(ctx, t) {
  ctx.fillStyle = C.chart;
  ctx.fillRect(0, 0, W, H);
  drawFog(ctx, t, "screen", 0.1);

  const frozen = t >= FREEZE_T;
  const drawCard = (i, x, y, scale, rot, a) => {
    const { cv } = cardCanvas[i];
    ctx.save();
    ctx.translate(x, y);
    ctx.rotate(rot);
    ctx.scale(scale, scale);
    ctx.globalAlpha = a;
    ctx.shadowColor = "rgba(0,0,0,0.55)";
    ctx.shadowBlur = 40;
    ctx.shadowOffsetY = 18;
    ctx.drawImage(cv, -cv.width / 2, -cv.height / 2);
    ctx.restore();
  };

  if (!frozen) {
    const idx = Math.min(CARDS.length - 1, Math.floor((t - CARD_T0) / CARD_DT));
    const local = (t - CARD_T0) - idx * CARD_DT;
    // previous two cards drift behind (parallax stack)
    for (let k = Math.max(0, idx - 2); k < idx; k++) {
      const age = idx - k;
      const px = W * (0.5 - age * 0.24), py = H * (0.5 - age * 0.07);
      drawCard(k, px + (k % 2 ? -30 : 30), py, 0.72 - age * 0.12, (k % 2 ? -1 : 1) * 0.05 * age, 0.5 - age * 0.16);
    }
    if (idx < CARDS.length && t >= CARD_T0) {
      const p = seg(local, 0, CARD_IN, EASE.outCubic);
      const x = lerp(W * 1.15, W * 0.5, p);
      const y = H * 0.5 + Math.sin(idx * 2.1) * 26;
      const rot = lerp(0.10, (idx % 2 ? -0.018 : 0.022), p);
      drawCard(idx, x, y, lerp(0.9, 0.8, p), rot, p);
      // stamp
      alpha(ctx, seg(local, 0.1, 0.4), () => {
        monoTag(ctx, CARDS[idx].stamp, W * 0.5 - 280, H * 0.94, "rgba(154,165,168,0.85)", 16, "left");
        monoTag(ctx, `ARCHIVE ${String(idx + 1).padStart(2, "0")} / ${String(CARDS.length).padStart(2, "0")}`, W * 0.5 + 280, H * 0.94, "rgba(53,208,165,0.8)", 16, "right");
      });
    }
  } else {
    // freeze tableau: 5 cards scattered, then scan
    const layout = [
      { x: 0.30, y: 0.42, s: 0.62, r: -0.05 }, { x: 0.52, y: 0.55, s: 0.7, r: 0.02 },
      { x: 0.70, y: 0.40, s: 0.58, r: 0.06 }, { x: 0.42, y: 0.62, s: 0.6, r: -0.025 },
      { x: 0.62, y: 0.58, s: 0.66, r: 0.035 },
    ];
    const order = [1, 3, 5, 4, 6];
    layout.forEach((L, i) => drawCard(order[i], W * L.x, H * L.y, L.s, L.r, 0.92));

    const scanP = seg(t, SCAN_T0, SCAN_T1, EASE.inOutCubic);
    if (scanP > 0) {
      const bx = lerp(-120, W + 120, scanP);
      // beam
      const bg = ctx.createLinearGradient(bx - 260, 0, bx + 8, 0);
      bg.addColorStop(0, "rgba(53,208,165,0)");
      bg.addColorStop(0.85, "rgba(53,208,165,0.13)");
      bg.addColorStop(1, "rgba(53,208,165,0.4)");
      ctx.fillStyle = bg;
      ctx.fillRect(bx - 260, 0, 268, H);
      ctx.fillStyle = "rgba(234,255,247,0.65)";
      ctx.fillRect(bx - 1.5, 0, 1.5, H);
      // revealed structure marks behind the beam
      alpha(ctx, 0.9, () => {
        ctx.strokeStyle = C.signal; ctx.fillStyle = C.signal;
        ctx.lineWidth = 1.4;
        const marks = [
          { x: 0.30, y: 0.36, w: 150 }, { x: 0.52, y: 0.47, w: 210 }, { x: 0.70, y: 0.34, w: 130 },
          { x: 0.42, y: 0.70, w: 170 }, { x: 0.62, y: 0.50, w: 190 }, { x: 0.55, y: 0.68, w: 120 },
        ];
        marks.forEach((m, i) => {
          const mx = W * m.x;
          if (bx > mx) {
            const reveal = seg(bx - mx, 0, 120);
            ctx.globalAlpha = reveal;
            ctx.strokeRect(mx - m.w / 2, H * m.y - 16, m.w, 30);
            monoTag(ctx, ["ENTITY", "RELATION", "FORMULA", "CONCEPT", "SOURCE", "METHOD"][i], mx - m.w / 2, H * m.y - 24, C.signal, 13);
            ctx.globalAlpha = 1;
          }
        });
      });
      monoTag(ctx, `SCANNING ARCHIVE · ${Math.floor(scanP * 47)} OBJECTS`, W * 0.5, H * 0.06, "rgba(53,208,165,0.9)", 16, "center");
    } else {
      monoTag(ctx, "ARCHIVE FROZEN · 0.0s", W * 0.5, H * 0.06, "rgba(154,165,168,0.6)", 15, "center");
    }
  }
}

/* ============================================================= S3 · MAP */

const MAP_NODES = [
  { x: 0.16, y: 0.72, zh: "射影平面", en: "PROJECTIVE PLANE" },
  { x: 0.28, y: 0.52, zh: "齐次坐标", en: "HOMOGENEOUS" },
  { x: 0.42, y: 0.60, zh: "交比", en: "CROSS RATIO", current: true },
  { x: 0.55, y: 0.44, zh: "调和分割", en: "HARMONIC" },
  { x: 0.66, y: 0.58, zh: "对偶原理", en: "DUALITY" },
  { x: 0.77, y: 0.40, zh: "二次曲线", en: "CONICS" },
  { x: 0.87, y: 0.54, zh: "配极变换", en: "POLARITY" },
];
let mapPathPts = null;

function buildMapPath() {
  // catmull-rom through nodes → sampled polyline
  const P = MAP_NODES.map((n) => ({ x: n.x * W, y: n.y * H }));
  const pts = [];
  for (let i = 0; i < P.length - 1; i++) {
    const p0 = P[Math.max(0, i - 1)], p1 = P[i], p2 = P[i + 1], p3 = P[Math.min(P.length - 1, i + 2)];
    for (let u = 0; u <= 24; u++) {
      const t2 = u / 24, tt = t2 * t2, ttt = tt * t2;
      pts.push({
        x: 0.5 * ((2 * p1.x) + (-p0.x + p2.x) * t2 + (2 * p0.x - 5 * p1.x + 4 * p2.x - p3.x) * tt + (-p0.x + 3 * p1.x - 3 * p2.x + p3.x) * ttt),
        y: 0.5 * ((2 * p1.y) + (-p0.y + p2.y) * t2 + (2 * p0.y - 5 * p1.y + 4 * p2.y - p3.y) * tt + (-p0.y + 3 * p1.y - 3 * p2.y + p3.y) * ttt),
      });
    }
  }
  // cumulative length
  let L = 0;
  pts.forEach((p, i) => { if (i) { L += Math.hypot(p.x - pts[i - 1].x, p.y - pts[i - 1].y); } p.d = L; });
  mapPathPts = { pts, total: L };
}

function drawPartialPath(ctx, pathData, progress, style, width, dash) {
  const { pts, total } = pathData;
  const target = total * clamp01(progress);
  ctx.save();
  ctx.strokeStyle = style;
  ctx.lineWidth = width;
  if (dash) ctx.setLineDash(dash);
  ctx.beginPath();
  let started = false;
  for (let i = 0; i < pts.length; i++) {
    if (pts[i].d > target) break;
    if (!started) { ctx.moveTo(pts[i].x, pts[i].y); started = true; }
    else ctx.lineTo(pts[i].x, pts[i].y);
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
    // lat/long curves
    ctx.strokeStyle = "rgba(154,165,168,0.07)";
    for (let i = 0; i < 5; i++) { ctx.beginPath(); ctx.ellipse(W / 2, H / 2, 400 + i * 220, 240 + i * 130, 0, 0, Math.PI * 2); ctx.stroke(); }
  });
}

function mapWorld(ctx, t, camera) {
  // camera: {cx, cy, scale} normalized-world space
  ctx.save();
  ctx.translate(W / 2, H / 2);
  ctx.scale(camera.scale, camera.scale);
  ctx.translate(-camera.cx * W, -camera.cy * H);

  chartGrid(ctx, 1);

  // cards arriving → nodes
  const arrive = seg(t, 13.2, 16.0, EASE.outCubic);
  MAP_NODES.forEach((n, i) => {
    const p = seg(t, 13.2 + i * 0.22, 13.2 + i * 0.22 + 1.1, EASE.outCubic);
    // card flies from scattered position to node
    const scatter = { x: (0.2 + 0.6 * hash2(i, 3)) * W, y: (0.25 + 0.5 * hash2(i, 9)) * H };
    const nx = n.x * W, ny = n.y * H;
    const x = lerp(scatter.x, nx, p), y = lerp(scatter.y, ny, p);
    if (p < 1) {
      const { cv } = cardCanvas[i % cardCanvas.length];
      ctx.save();
      ctx.globalAlpha = (1 - p) * 0.85;
      ctx.translate(x, y);
      ctx.rotate((hash2(i, 5) - 0.5) * 0.2 * (1 - p));
      const s = lerp(0.34, 0.02, p);
      ctx.scale(s, s);
      ctx.drawImage(cv, -cv.width / 2, -cv.height / 2);
      ctx.restore();
    }
    if (p > 0.55) {
      const na = seg(p, 0.55, 1);
      ctx.save();
      ctx.globalAlpha = na;
      signalDot(ctx, nx, ny, n.current ? 6 : 4, n.current);
      ctx.font = F.mono(15, 500);
      ctx.fillStyle = "rgba(188,205,214,0.85)";
      try { ctx.letterSpacing = "2px"; } catch (e) {}
      ctx.fillText(n.en, nx + 16, ny - 10);
      ctx.font = F.song(17, 400);
      ctx.fillStyle = "rgba(246,242,234,0.75)";
      ctx.fillText(n.zh, nx + 16, ny + 12);
      ctx.restore();
    }
  });

  // route draws through nodes
  const routeP = seg(t, 16.2, 19.0, EASE.inOutCubic);
  if (routeP > 0) {
    drawPartialPath(ctx, mapPathPts, routeP, "rgba(53,208,165,0.75)", 2, [8, 7]);
    // lamps along the way
    [0.14, 0.5, 0.86].forEach((f, i) => {
      if (routeP > f) {
        const pt = mapPathPts.pts.find((p) => p.d >= mapPathPts.total * f) || mapPathPts.pts[mapPathPts.pts.length - 1];
        amberDot(ctx, pt.x, pt.y, 4);
      }
    });
  }
  ctx.restore();
}

function s3(ctx, t) {
  ctx.fillStyle = C.chart;
  ctx.fillRect(0, 0, W, H);
  drawFog(ctx, t, "screen", 0.12);
  const cam = { cx: 0.5, cy: 0.52, scale: lerp(1.0, 1.14, seg(t, 13.2, 21, EASE.inOutCubic)) };
  mapWorld(ctx, t, cam);
  if (t > 19.0) {
    alpha(ctx, seg(t, 19.1, 19.8), () => {
      ctx.textAlign = "center";
      ctx.font = F.serif(52, true);
      ctx.fillStyle = "rgba(246,242,234,0.95)";
      ctx.fillText("Turn information into a journey.", W / 2, H * 0.86);
      ctx.textAlign = "left";
    });
  }
  monoTag(ctx, "CHART OF UNDERSTANDING · NO.7", W * 0.5, H * 0.07, "rgba(154,165,168,0.55)", 15, "center");
}

/* ============================================================ S4 · ORBIT */

const ZOOM_NODE = MAP_NODES[2]; // 交比

function s4(ctx, t) {
  if (t < 23.0) {
    // continuous zoom into the current node; fog swallows the seam
    const z = seg(t, 21.0, 22.6, EASE.inCubic);
    const scale = lerp(1.14, 9, z);
    const cam = {
      cx: lerp(0.5, ZOOM_NODE.x, z), cy: lerp(0.52, ZOOM_NODE.y, z), scale,
    };
    ctx.fillStyle = C.chart;
    ctx.fillRect(0, 0, W, H);
    mapWorld(ctx, 20.99, cam); // hold the drawn route, camera moves
    // fog rises into the lens
    drawFog(ctx, t, "screen", 0.15 + z * 0.85);
    // emerge into the real product
    const em = seg(t, 22.3, 23.0, EASE.outCubic);
    if (em > 0) {
      alpha(ctx, em, () => drawShot(ctx, IMG.roadmapDark, { cx: 0.5, cy: 0.5, scale: 1.14 }));
      drawFog(ctx, t * 0.8, "screen", (1 - em) * 0.8);
    }
    return;
  }

  if (t < 26.6) {
    // real roadmap, slow dolly along the path + instrument annotations
    const p = seg(t, 23.0, 26.6, EASE.inOutCubic);
    const cam = { cx: lerp(0.5, 0.485, p), cy: lerp(0.47, 0.60, p), scale: lerp(1.16, 1.02, p) };
    drawShot(ctx, IMG.roadmapDark, cam);
    const an = seg(t, 23.6, 24.4);
    alpha(ctx, an * 0.9, () => {
      monoTag(ctx, "ROUTE · 射影几何", W * 0.055, H * 0.16, "rgba(53,208,165,0.9)", 17);
      monoTag(ctx, "6 NODES · 2 LIT", W * 0.055, H * 0.16 + 26, "rgba(154,165,168,0.75)", 14);
      // callout glued to the current node card (image-space rect)
      const p0 = shotPoint(IMG.roadmapDark, cam, 0.385, 0.585);
      const p1 = shotPoint(IMG.roadmapDark, cam, 0.685, 0.725);
      ctx.strokeStyle = "rgba(53,208,165,0.6)";
      ctx.lineWidth = 1.2;
      ctx.strokeRect(p0.x, p0.y, p1.x - p0.x, p1.y - p0.y);
      ctx.beginPath(); ctx.moveTo(p1.x + 26, p1.y + 60); ctx.lineTo(p1.x, p1.y); ctx.stroke();
      monoTag(ctx, "CURRENT → 交比与调和分割", p1.x + 30, p1.y + 66, "rgba(53,208,165,0.9)", 15);
    });
    return;
  }

  if (t < 28.6) {
    // occlusion wipes: lesson → conversation
    const p = seg(t, 26.6, 27.4);
    if (p < 1) {
      drawShot(ctx, IMG.roadmapDark, { cx: 0.485, cy: 0.60, scale: 1.02 });
      wipeShot(ctx, IMG.lessonDark, p, 0.05);
    } else {
      const p2 = seg(t, 27.5, 28.3);
      drawShot(ctx, IMG.lessonDark, { cx: 0.5, cy: 0.5, scale: 1.03 });
      if (p2 > 0) wipeShot(ctx, IMG.convLight, p2, -0.04);
    }
    return;
  }

  // conversation with pedagogy tags
  const p = seg(t, 28.6, 32.0, EASE.inOutCubic);
  drawShot(ctx, IMG.convLight, { cx: 0.52, cy: 0.5, scale: lerp(1.03, 1.10, p) });
  const tags = [
    { en: "EXPLAIN", zh: "解释", y: 0.24, tx: 0.40, ty: 0.20 },
    { en: "ASK", zh: "追问", y: 0.36, tx: 0.42, ty: 0.385 },
    { en: "HINT", zh: "提示", y: 0.48, tx: 0.44, ty: 0.475 },
    { en: "EXAMPLE", zh: "举例", y: 0.60, tx: 0.45, ty: 0.555 },
    { en: "CHALLENGE", zh: "挑战", y: 0.72, tx: 0.47, ty: 0.625 },
  ];
  tags.forEach((tag, i) => {
    const a = seg(t, 28.9 + i * 0.55, 29.3 + i * 0.55, EASE.outCubic);
    if (a <= 0) return;
    alpha(ctx, a, () => {
      const x = W * 0.075, y = H * tag.y;
      ctx.fillStyle = C.signal;
      ctx.fillRect(x, y - 13, 3, 18);
      monoTag(ctx, tag.en, x + 14, y, "rgba(27,29,32,0.9)", 17);
      ctx.font = F.song(15, 400);
      ctx.fillStyle = "rgba(27,29,32,0.55)";
      ctx.fillText(tag.zh, x + 14, y + 20);
      // leader line into the conversation column
      ctx.strokeStyle = "rgba(53,208,165,0.55)";
      ctx.lineWidth = 1;
      ctx.beginPath(); ctx.moveTo(x + 120, y - 4); ctx.lineTo(W * tag.tx, H * tag.ty); ctx.stroke();
      signalDot(ctx, W * tag.tx, H * tag.ty, 2.5, false);
    });
  });
}

/* ============================================================ S5 · THINK */

/* crop rects in convLight image space (3840×2160) */
const CROP = {
  probe: { cx: 0.55, cy: 0.40, scale: 2.2 },       // AI probe paragraph
  probeFrom: { cx: 0.50, cy: 0.33, scale: 1.55 },
  answer: { cx: 0.70, cy: 0.33, scale: 2.6 },       // learner bubble
  confirm: { cx: 0.55, cy: 0.585, scale: 2.0 },     // confirmation + citation
};

function s5(ctx, t) {
  // paper-like quiet ground under the crops
  ctx.fillStyle = "#f2eee6";
  ctx.fillRect(0, 0, W, H);

  if (t < 36.0) {
    // macro on the AI probe question
    const p = seg(t, 32.0, 35.8, EASE.inOutCubic);
    const drift = {
      cx: lerp(CROP.probeFrom.cx, CROP.probe.cx, p),
      cy: lerp(CROP.probeFrom.cy, CROP.probe.cy, p),
      scale: lerp(CROP.probeFrom.scale, CROP.probe.scale, p),
    };
    drawShot(ctx, IMG.convLight, drift);
    // circular scan over the key phrase 「保持不变」 in the probe line
    const scan = seg(t, 33.6, 35.2);
    if (scan > 0) {
      const pt = shotPoint(IMG.convLight, drift, 0.528, 0.349);
      const R = 62 * (pt.cover / 1.1);
      ctx.save();
      ctx.strokeStyle = "#0d9a72";
      ctx.lineWidth = 2.4;
      const sweep = EASE.inOutCubic(scan) * Math.PI * 2;
      ctx.beginPath(); ctx.ellipse(pt.x, pt.y, R * 1.5, R * 0.5, -0.02, -Math.PI / 2, -Math.PI / 2 + sweep); ctx.stroke();
      if (scan >= 1) {
        alpha(ctx, seg(t, 35.0, 35.4), () => {
          ctx.setLineDash([5, 4]);
          ctx.beginPath(); ctx.ellipse(pt.x, pt.y, R * 1.5, R * 0.5, -0.02, 0, Math.PI * 2); ctx.stroke();
          ctx.setLineDash([]);
          monoTag(ctx, "KEYWORD · 保持不变", pt.x - R * 1.5, pt.y - R * 0.72, "#0d7a5c", 16);
        });
      }
      ctx.restore();
    }
    return;
  }

  if (t < 38.6) {
    // keyword lifts out and becomes a node; the follow-up appears beneath
    const p = seg(t, 36.0, 37.2, EASE.outCubic);
    drawFog(ctx, t, "multiply", 0.06);
    const x0 = W * 0.45, y0 = H * 0.39;   // lifts from the circled phrase
    const x1 = W * 0.50, y1 = H * 0.38;
    const x = lerp(x0, x1, p), y = lerp(y0, y1, p);
    // origin UI ghost fades to a quiet emboss, not to nothing
    alpha(ctx, 1 - p * 0.72, () => drawShot(ctx, IMG.convLight, CROP.probe));
    // the node
    alpha(ctx, seg(t, 36.3, 36.9), () => {
      signalDot(ctx, x, y, 6, true);
      ctx.font = F.song(30, 400);
      ctx.fillStyle = C.ink;
      ctx.fillText("「保持不变」", x + 26, y + 4);
      monoTag(ctx, "NEW NODE · FROM THE LEARNER'S OWN WORDS", x + 26, y + 30, "rgba(13,122,92,0.8)", 13);
    });
    // follow-up question fades in below
    alpha(ctx, seg(t, 37.2, 38.2), () => {
      ctx.font = F.song(26, 400);
      ctx.fillStyle = "rgba(27,29,32,0.85)";
      ctx.fillText("——如果长度会变，什么关系是投影拿不走的？", W * 0.24, H * 0.62);
      monoTag(ctx, "ORBIT ASKS AGAIN · NO ANSWER GIVEN", W * 0.24, H * 0.62 + 34, "rgba(138,133,120,0.8)", 13);
    });
    return;
  }

  if (t < 40.0) {
    // mastery: a node lights up and connects back to the route
    const p = seg(t, 38.6, 39.6, EASE.outCubic);
    ctx.fillStyle = "#f2eee6";
    ctx.fillRect(0, 0, W, H);
    drawFog(ctx, t, "multiply", 0.05);
    const nx = W * 0.5, ny = H * 0.44;
    // the lifted keyword stays, now part of the chart
    alpha(ctx, 0.55, () => {
      signalDot(ctx, nx, H * 0.26, 3.5, false);
      ctx.font = F.song(19, 400);
      ctx.fillStyle = "rgba(27,29,32,0.6)";
      ctx.fillText("「保持不变」", nx + 18, H * 0.26 + 6);
      ctx.strokeStyle = "rgba(53,208,165,0.4)";
      ctx.setLineDash([3, 4]);
      ctx.beginPath(); ctx.moveTo(nx, H * 0.26 + 8); ctx.lineTo(nx, ny - 14); ctx.stroke();
      ctx.setLineDash([]);
    });
    // tiny route echo
    ctx.save();
    ctx.strokeStyle = "rgba(138,133,120,0.55)";
    ctx.lineWidth = 1.2;
    ctx.setLineDash([6, 6]);
    ctx.beginPath(); ctx.moveTo(W * 0.2, H * 0.72); ctx.quadraticCurveTo(W * 0.5, H * 0.60, W * 0.8, H * 0.70); ctx.stroke();
    ctx.restore();
    const lampP = seg(t, 39.0, 39.5, EASE.outBack);
    if (lampP > 0) {
      signalDot(ctx, nx, ny, 6 * lampP, true);
      alpha(ctx, lampP, () => {
        monoTag(ctx, "MASTERY +0.12", nx + 24, ny + 4, "#0d7a5c", 16);
        monoTag(ctx, "NODE LIT · 交比", nx + 24, ny + 28, "rgba(138,133,120,0.9)", 13);
      });
      // connect down to route
      alpha(ctx, seg(t, 39.3, 39.9), () => {
        ctx.strokeStyle = "rgba(53,208,165,0.6)";
        ctx.setLineDash([4, 5]);
        ctx.beginPath(); ctx.moveTo(nx, ny + 10); ctx.quadraticCurveTo(nx + 40, ny + 160, W * 0.52, H * 0.685); ctx.stroke();
        ctx.setLineDash([]);
      });
    }
    return;
  }

  // 40.0–42.0 — the quiet thesis
  ctx.fillStyle = "#0a0c0e";
  ctx.fillRect(0, 0, W, H);
  ctx.textAlign = "center";
  alpha(ctx, Math.min(seg(t, 40.1, 40.5), 1), () => {
    ctx.font = F.serif(64, true);
    ctx.fillStyle = "rgba(246,242,234,0.6)";
    ctx.fillText("Not answers.", W / 2, H * 0.47);
  });
  alpha(ctx, seg(t, 41.0, 41.5), () => {
    ctx.font = F.serif(76, true);
    ctx.fillStyle = "#f6f2ea";
    ctx.fillText("Understanding.", W / 2, H * 0.60);
    ctx.strokeStyle = C.signal;
    ctx.lineWidth = 2;
    const w = 300;
    ctx.beginPath(); ctx.moveTo(W / 2 - w / 2, H * 0.60 + 26); ctx.lineTo(W / 2 + w / 2, H * 0.60 + 26); ctx.stroke();
  });
  ctx.textAlign = "left";
}

/* =========================================================== S6 · WORLD */

const SUBJECTS = [
  { name: "PHYSICS", az: 0.14, y: 0.34 },
  { name: "HISTORY", az: 0.28, y: 0.62 },
  { name: "BIOLOGY", az: 0.42, y: 0.30 },
  { name: "CODE", az: 0.58, y: 0.58 },
  { name: "PHILOSOPHY", az: 0.72, y: 0.36 },
  { name: "MUSIC THEORY", az: 0.86, y: 0.60 },
];

function s6(ctx, t) {
  if (t < 45.0) {
    // deconstruction: UI dissolves into paper, paper into stars
    const p = seg(t, 42.0, 45.0, EASE.inOutCubic);
    ctx.fillStyle = "#0a0d10";
    ctx.fillRect(0, 0, W, H);
    // UI sinks & fades quickly into the dark
    alpha(ctx, Math.pow(1 - p, 1.5), () => drawShot(ctx, IMG.convLight, { cx: 0.52, cy: lerp(0.5, 0.56, p), scale: lerp(1.1, 1.25, p) }));
    // paper cards lift off toward the sky, carrying a few lines of text
    const rng = mulberry32(77);
    for (let i = 0; i < 10; i++) {
      const sx = 0.25 + rng() * 0.5, sy = 0.2 + rng() * 0.6;
      const lp = seg(p, rng() * 0.5, 1);
      const x = W * sx + (rng() - 0.5) * 140 * lp;
      const y = H * sy - lp * H * 0.5;
      const cw = 150 * lerp(1, 0.35, lp), ch = 96 * lerp(1, 0.35, lp);
      ctx.save();
      ctx.globalAlpha = (1 - lp) * 0.95 * (p > 0.05 ? 1 : 0);
      ctx.translate(x, y);
      ctx.rotate((rng() - 0.5) * 0.6 * lp);
      ctx.fillStyle = C.paper;
      ctx.fillRect(-cw / 2, -ch / 2, cw, ch);
      // ghost text lines + one signal mark
      ctx.strokeStyle = "rgba(43,44,48,0.5)";
      ctx.lineWidth = 1;
      for (let li = 0; li < 3; li++) {
        const lw = cw * (0.5 + rng() * 0.35);
        ctx.beginPath(); ctx.moveTo(-cw / 2 + 10, -ch / 4 + li * ch / 4); ctx.lineTo(-cw / 2 + 10 + lw, -ch / 4 + li * ch / 4); ctx.stroke();
      }
      if (i % 3 === 0) { ctx.fillStyle = "rgba(13,154,114,0.7)"; ctx.fillRect(cw / 2 - 16, -ch / 2 + 8, 6, 6); }
      ctx.restore();
      // becomes a star
      if (lp > 0.85) {
        alpha(ctx, (lp - 0.85) / 0.15, () => signalDot(ctx, x, y, 1.8, false));
      }
    }
    drawFog(ctx, t, "screen", 0.25 + p * 0.25);
    return;
  }

  // the beacon sea
  ctx.fillStyle = "#080b0e";
  ctx.fillRect(0, 0, W, H);
  drawFog(ctx, t * 0.6, "screen", 0.22);
  const horizon = H * 0.66;
  // sea
  const sea = ctx.createLinearGradient(0, horizon, 0, H);
  sea.addColorStop(0, "rgba(23,28,33,0.9)");
  sea.addColorStop(1, "#05070a");
  ctx.fillStyle = sea;
  ctx.fillRect(0, horizon, W, H - horizon);
  ctx.strokeStyle = "rgba(188,205,214,0.28)";
  ctx.lineWidth = 1;
  ctx.beginPath(); ctx.moveTo(0, horizon + 0.5); ctx.lineTo(W, horizon + 0.5); ctx.stroke();

  // beacon: a point + rotating beam
  const bx = W * 0.5, by = horizon - 10;
  const sweepT = seg(t, 45.0, 49.0, EASE.linear);
  const beamA = lerp(-2.6, 2.6, sweepT); // world azimuth sweep
  ctx.save();
  // beam (two soft wedges)
  for (const dir of [0, Math.PI]) {
    const a = beamA + dir;
    const grad = ctx.createLinearGradient(bx, by, bx + Math.cos(a) * 900, by + Math.sin(a) * 900);
    grad.addColorStop(0, "rgba(53,208,165,0.35)");
    grad.addColorStop(1, "rgba(53,208,165,0)");
    ctx.fillStyle = grad;
    ctx.beginPath();
    ctx.moveTo(bx, by);
    ctx.arc(bx, by, 950, a - 0.045, a + 0.045);
    ctx.closePath();
    ctx.fill();
  }
  signalDot(ctx, bx, by, 5, true);
  ctx.restore();
  monoTag(ctx, "BEACON · ANY SUBJECT", bx, by + 40, "rgba(154,165,168,0.7)", 14, "center");

  // subjects appear as the beam passes their azimuth
  SUBJECTS.forEach((s, i) => {
    const show = seg(sweepT, s.az - 0.04, s.az + 0.02) * (1 - seg(sweepT, s.az + 0.18, s.az + 0.3) * 0.55);
    if (show <= 0) return;
    alpha(ctx, show, () => {
      const x = W * (0.12 + s.az * 0.76), y = H * s.y;
      monoTag(ctx, s.name, x, y, "rgba(246,242,234,0.95)", 30);
      monoTag(ctx, `${(s.az * 360).toFixed(1)}° · CHART ${String(i + 1).padStart(2, "0")}`, x, y + 24, "rgba(53,208,165,0.85)", 13);
      signalDot(ctx, x - 18, y - 8, 2.5, false);
    });
  });

  if (t > 49.0) {
    alpha(ctx, seg(t, 49.2, 50.0), () => {
      ctx.textAlign = "center";
      ctx.font = F.serif(50, true);
      ctx.fillStyle = "rgba(246,242,234,0.95)";
      ctx.fillText("Any subject can become a journey.", W / 2, H * 0.5);
      ctx.textAlign = "left";
    });
  }
}

/* =========================================================== S7 · BRAND */

const ORBIT = { cx: W * 0.5, cy: H * 0.47, rx: W * 0.24, ry: H * 0.15, tilt: -0.22 };

function orbitPoint(a) {
  const x = Math.cos(a) * ORBIT.rx, y = Math.sin(a) * ORBIT.ry;
  return {
    x: ORBIT.cx + x * Math.cos(ORBIT.tilt) - y * Math.sin(ORBIT.tilt),
    y: ORBIT.cy + x * Math.sin(ORBIT.tilt) + y * Math.cos(ORBIT.tilt),
  };
}

function s7(ctx, t) {
  ctx.fillStyle = "#05070a";
  ctx.fillRect(0, 0, W, H);
  drawFog(ctx, t * 0.5, "screen", 0.08);

  // the dot
  const appear = seg(t, 52.0, 52.4, EASE.outCubic);
  const draw = seg(t, 53.2, 55.8, EASE.inOutCubic);
  const a0 = -Math.PI * 0.5;
  const head = a0 + draw * Math.PI * 2;
  const hp = orbitPoint(head);

  if (draw > 0) {
    // trail
    ctx.save();
    ctx.strokeStyle = C.signal;
    ctx.lineWidth = 2;
    ctx.beginPath();
    for (let a = a0; a <= head; a += 0.02) {
      const p = orbitPoint(a);
      a === a0 ? ctx.moveTo(p.x, p.y) : ctx.lineTo(p.x, p.y);
    }
    ctx.stroke();
    ctx.restore();
    // amber periapsis — the product's lamp, once
    const pp = orbitPoint(a0 + Math.PI * 0.5);
    amberDot(ctx, pp.x, pp.y, 4);
  }

  if (appear > 0) {
    const px = draw > 0 ? hp : orbitPoint(a0);
    alpha(ctx, appear, () => signalDot(ctx, px.x, px.y, 5.5, true));
    // initial ping ring
    const ring = seg(t, 52.05, 53.0, EASE.outCubic);
    if (ring < 1) alpha(ctx, (1 - ring) * 0.6, () => {
      ctx.strokeStyle = C.signal;
      ctx.lineWidth = 1.5;
      ctx.beginPath(); ctx.arc(orbitPoint(a0).x, orbitPoint(a0).y, 10 + ring * 200, 0, Math.PI * 2); ctx.stroke();
    });
  }

  // wordmark
  const wm = seg(t, 55.9, 56.8, EASE.outCubic);
  if (wm > 0) {
    alpha(ctx, wm, () => {
      ctx.textAlign = "center";
      ctx.font = F.song(132, 400);
      ctx.fillStyle = "#f6f2ea";
      ctx.fillText("学 涯", W / 2, H * 0.485);
      ctx.font = F.mono(30, 500);
      try { ctx.letterSpacing = "14px"; } catch (e) {}
      ctx.fillStyle = "rgba(246,242,234,0.75)";
      ctx.fillText("ORBIT", W / 2 + 7, H * 0.485 + 66);
      try { ctx.letterSpacing = "0px"; } catch (e) {}
      ctx.textAlign = "left";
    });
  }
  const tag = seg(t, 57.6, 58.4, EASE.outCubic);
  if (tag > 0) {
    alpha(ctx, tag, () => {
      ctx.textAlign = "center";
      ctx.font = F.serif(30, true);
      ctx.fillStyle = "rgba(246,242,234,0.8)";
      ctx.fillText("Find your way through knowledge.", W / 2, H * 0.72);
      ctx.textAlign = "left";
    });
  }
}

/* ================================================================ frame */

function renderFrameAbs(ctx, t) {
  ctx.clearRect(0, 0, W, H);
  if (t < 5.5) s1(ctx, t);
  else if (t < 13.2) s2(ctx, t);
  else if (t < 21.0) s3(ctx, t);
  else if (t < 32.0) s4(ctx, t);
  else if (t < 42.0) s5(ctx, t);
  else if (t < 52.0) s6(ctx, t);
  else s7(ctx, t);
  drawGrain(ctx, t, 0.5);
  vignette(ctx, 0.32);
}

window.__film = {
  ready: (async () => {
    fogTex = makeFogTex();
    grainTiles = [makeGrainTile(11), makeGrainTile(22), makeGrainTile(33), makeGrainTile(44)];
    buildCards();
    buildMapPath();
    await loadImages();
    await document.fonts.ready;
    return true;
  })(),
  render(t) {
    const cv = document.getElementById("stage");
    const ctx = cv.getContext("2d");
    renderFrameAbs(ctx, t);
  },
  DUR, FPS,
};
