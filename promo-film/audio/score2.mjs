/** ORBIT v2 — 54s stereo 48kHz procedural score, overlap retime.
 * QUESTION 0–3.6; TOO MUCH 3.4–9.6; STRUCTURE 9.6–15.6;
 * ROUTE 15.6–20.4; ENTER 20.3–25.2; LEARNING 25–34.4;
 * UNDERSTANDING 34.4–38.6; EXPAND 38.6–46; ORBIT 46–54.
 * Near silence 36.6–38.2; resolve 51–54. No samples.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SR = 48000, DUR = 54, N = SR * DUR;
const OUT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "score2.wav");

const L = new Float32Array(N), R = new Float32Array(N);

function mulberry32(seed) {
  let a = seed >>> 0;
  return function () {
    a |= 0; a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const seg = (t, a, b) => clamp01((t - a) / (b - a));
const smooth = (t) => t * t * (3 - 2 * t);

function lp(cutoff) {
  const rc = 1 / (2 * Math.PI * cutoff), dt = 1 / SR;
  const a = dt / (rc + dt);
  let y = 0;
  return (x) => (y += a * (x - y));
}
function hp(cutoff) {
  const rc = 1 / (2 * Math.PI * cutoff), dt = 1 / SR;
  const a = rc / (rc + dt);
  let y = 0, xp = 0;
  return (x) => { const v = a * (y + x - xp); y = v; xp = x; return v; };
}

function addAt(t0, fn, len, gain, pan = 0) {
  const start = Math.floor(t0 * SR);
  const count = Math.min(Math.floor(len * SR), N - start);
  if (count <= 0) return;
  const gl = gain * Math.cos((pan + 1) * Math.PI / 4), gr = gain * Math.sin((pan + 1) * Math.PI / 4);
  for (let i = 0; i < count; i++) {
    const v = fn(i / SR, i);
    L[start + i] += v * gl;
    R[start + i] += v * gr;
  }
}

/* ------------------------------------------------------------------ wind */
{
  const rng = mulberry32(41);
  const lpL = lp(320), lpR = lp(280);
  let bL = 0, bR = 0;
  for (let i = 0; i < N; i++) {
    const t = i / SR;
    bL = (bL + 0.02 * (rng() * 2 - 1)) / 1.02;
    bR = (bR + 0.02 * (rng() * 2 - 1)) / 1.02;
    const breathe = 0.55 + 0.45 * Math.sin(t * 0.42 + 1.3) * Math.sin(t * 0.11);
    const dropA = 1 - 0.94 * smooth(seg(t, 36.6, 36.9)) * (1 - smooth(seg(t, 38.2, 38.8)));
    const dropB = 1 - 0.85 * smooth(seg(t, 45.7, 46.3)) * (1 - smooth(seg(t, 47.0, 48.2)));
    const room = 0.45 + 0.55 * (1 - smooth(seg(t, 25.0, 26.0)) * (1 - smooth(seg(t, 33.8, 34.4)))); // drier in LEARNING
    const g = 0.16 * breathe * dropA * dropB * room;
    L[i] += lpL(bL) * g * 8;
    R[i] += lpR(bR) * g * 8;
  }
}

/* ------------------------------------------------------- pencil scratches */
{
  const rng = mulberry32(17);
  const hpF = hp(2200);
  for (let s = 0; s < 14; s++) {
    addAt(1.35 + s * 0.085, (t) => {
      const env = Math.exp(-t * 26) * (0.6 + 0.4 * Math.sin(t * 240));
      return hpF(rng() * 2 - 1) * env;
    }, 0.09, 0.16, (rng() - 0.5) * 0.5);
  }
}

/* ----------------------------------------------------------------- pings */
function ping(t0, freq, gain, echoes, pan = 0) {
  for (let e = 0; e < echoes; e++) {
    const dt = e * 0.42, g = gain * Math.pow(0.45, e);
    addAt(t0 + dt, (t) => {
      const env = Math.exp(-t * 7.5);
      return Math.sin(2 * Math.PI * freq * (1 + 0.002 * Math.sin(t * 30)) * t) * env * Math.min(1, t / 0.004);
    }, 1.4, g, pan + e * 0.12);
    addAt(t0 + dt, (t) => Math.sin(2 * Math.PI * freq * 2.01 * t) * Math.exp(-t * 11), 1.0, g * 0.25, pan);
  }
}
ping(2.15, 780, 0.5, 3, 0.1);      // the dot is born
ping(18.9, 660, 0.3, 2, -0.1);     // route named
ping(46.2, 780, 0.55, 2, 0);       // ORBIT — driest

/* -------------------------------------------------------------- pulse */
function thump(t0, gain, freq = 52) {
  addAt(t0, (t) => Math.sin(2 * Math.PI * freq * Math.exp(-t * 6) * t) * Math.exp(-t * 9), 0.5, gain);
}
for (let t = 3.8; t < 9.2; t += 0.66) thump(t, 0.13, 56);          // TOO MUCH momentum
for (let t = 16.0; t < 20.0; t += 0.79) thump(t, 0.12);             // ROUTE
for (let t = 25.4; t < 33.8; t += 1.11) thump(t, 0.09, 50);         // LEARNING: calm 54bpm

/* ----------------------------------------------------------- paper reveals */
function rustle(t0, gain, seed) {
  const rng = mulberry32(seed);
  const hpF = hp(1400);
  addAt(t0, (t) => {
    const flap = 0.5 + 0.5 * Math.sin(t * 90 + rng() * 6);
    return hpF(rng() * 2 - 1) * Math.exp(-t * 16) * Math.min(1, t / 0.012) * flap;
  }, 0.35, gain, (rng() - 0.5) * 0.7);
}
[3.5, 3.65, 3.8, 3.85, 3.95, 4.1].forEach((t, i) => rustle(t, 0.24, 100 + i));
rustle(24.4, 0.26, 200);                                            // wipe into conversation

/* pulse whoosh at 3.4 (the scan pulse that finds the papers) */
{
  const rng = mulberry32(55);
  const s0 = Math.floor(3.4 * SR), s1 = Math.floor(4.4 * SR);
  let ph = 0;
  for (let i = s0; i < s1; i++) {
    const t = i / SR, p = (t - 3.4) / 1.0;
    const f = 90 + p * 480;
    ph += 2 * Math.PI * f / SR;
    const g = Math.sin(p * Math.PI) * 0.12;
    const n = (rng() * 2 - 1) * 0.05 * Math.sin(p * Math.PI);
    L[i] += Math.sin(ph) * g + n; R[i] += Math.sin(ph * 0.997) * g + n;
  }
}

/* --------------------------------------------------------- scan sweep 8.9 */
{
  const rng = mulberry32(99);
  const s0 = Math.floor(8.9 * SR), s1 = Math.floor(9.6 * SR);
  let ph = 0;
  for (let i = s0; i < s1; i++) {
    const t = i / SR, p = (t - 8.9) / 0.7;
    const f = 380 + p * 2400;
    ph += 2 * Math.PI * f / SR;
    const g = Math.sin(p * Math.PI);
    L[i] += Math.sin(ph) * 0.05 * g + (rng() * 2 - 1) * 0.028 * g;
    R[i] += Math.sin(ph) * 0.045 * g;
  }
}

/* -------------------------------------- extraction plucks (rising = order) */
function pluck(t0, freq, gain, pan) {
  addAt(t0, (t) => {
    const env = Math.exp(-t * 10);
    return (Math.sin(2 * Math.PI * freq * t) + 0.4 * Math.sin(2 * Math.PI * freq * 3 * t) * Math.exp(-t * 30)) * env;
  }, 0.7, gain, pan);
}
[0, 1, 2, 3, 4, 5].forEach((i) => pluck(10.05 + i * 0.8, 620 + i * 95, 0.16, (i - 2.5) * 0.25));

/* -------------------------------------------------------------- route ticks */
function tick(t0, gain, freq = 1900) {
  addAt(t0, (t) => Math.sin(2 * Math.PI * freq * t) * Math.exp(-t * 90) + (mulberry32(3)() * 2 - 1) * Math.exp(-t * 300) * 0.5, 0.08, gain);
}
[16.35, 16.95, 17.6, 18.3, 19.0].forEach((t, i) => tick(t, 0.14, 1400 + i * 140)); // node arrivals (hops)
tick(22.7, 0.16, 1600);   // UI settles
tick(30.1, 0.15, 1500);   // circle closes on the keyword
tick(32.3, 0.15, 1700);   // underline lands
tick(33.9, 0.12, 1200);  // confirmation
tick(37.7, 0.26, 900);    // the wooden click in the silence

/* ring riser + soft boom entering the product */
{
  const s0 = Math.floor(20.3 * SR), s1 = Math.floor(21.5 * SR);
  let ph = 0;
  for (let i = s0; i < s1; i++) {
    const t = i / SR, p = (t - 20.3) / 1.2;
    const f = 140 * Math.pow(2, p * 2.4);
    ph += 2 * Math.PI * f / SR;
    const g = p * p * 0.07;
    L[i] += Math.sin(ph) * g; R[i] += Math.sin(ph) * g;
  }
}
addAt(21.5, (t) => Math.sin(2 * Math.PI * 58 * Math.exp(-t * 4) * t) * Math.exp(-t * 5), 1.6, 0.32);

/* ------------------------------------------------------------------ pad */
function padNote(t0, t1, freq, gain, pan) {
  const start = Math.floor(t0 * SR), end = Math.min(Math.floor(t1 * SR), N);
  const len = end - start;
  if (len <= 0) return;
  for (let i = 0; i < len; i++) {
    const t = i / SR, u = i / len;
    const env = smooth(clamp01(u / 0.25)) * (1 - smooth(seg(u, 0.72, 1)));
    const v = Math.sin(2 * Math.PI * freq * t) * 0.5 + Math.sin(2 * Math.PI * freq * 1.005 * t) * 0.3 + Math.sin(2 * Math.PI * freq * 0.5 * t) * 0.2;
    L[start + i] += v * gain * env * Math.cos((pan + 1) * Math.PI / 4);
    R[start + i] += v * gain * env * Math.sin((pan + 1) * Math.PI / 4);
  }
}
// quiet ground under STRUCTURE→ROUTE (D A E)
padNote(11.8, 20.2, 146.83, 0.045, -0.3);
padNote(12.2, 20.2, 220.0, 0.04, 0.2);
padNote(12.6, 20.2, 329.63, 0.028, 0.45);
// mastery fifth — warm, brief
padNote(34.8, 36.3, 220.0, 0.05, -0.1);
padNote(34.85, 36.3, 329.63, 0.04, 0.15);
// EXPAND opens up (D A F#)
padNote(38.8, 46.0, 146.83, 0.05, -0.35);
padNote(38.8, 46.0, 220.0, 0.045, 0.1);
padNote(39.2, 46.0, 369.99, 0.03, 0.4);
// resolve chord at the wordmark
padNote(51.0, 54.0, 146.83, 0.06, -0.2);
padNote(51.1, 54.0, 220.0, 0.05, 0.15);
padNote(51.2, 54.0, 293.66, 0.04, 0.35);
padNote(51.3, 54.0, 369.99, 0.028, 0.5);

/* subject pings in EXPAND — sparse, high, spread */
[41.2, 41.7, 42.2, 42.7, 43.2, 43.7].forEach((t, i) =>
  ping(t, 1150 + i * 60, 0.07, 1, (i - 2.5) * 0.3));

/* beacon hum */
padNote(43.8, 45.8, 73.42, 0.05, 0.1);   // D2 hum under the beacon

/* orbit draw shimmer */
{
  const s0 = Math.floor(46.8 * SR), s1 = Math.floor(51.1 * SR);
  let ph = 0;
  for (let i = s0; i < s1; i++) {
    const t = i / SR, p = (t - 46.8) / 4.3;
    const f = 1800 + p * 1400;
    ph += 2 * Math.PI * f / SR;
    const g = 0.02 * Math.sin(p * Math.PI);
    L[i] += Math.sin(ph) * g;
    R[i] += Math.sin(ph * 1.003) * g;
  }
}

/* ------------------------------------------------------- master + write */
{
  let peak = 0;
  for (let i = 0; i < N; i++) {
    const fadeIn = Math.min(1, i / (SR * 0.4));
    const fadeOut = Math.min(1, (N - i) / (SR * 0.6));
    L[i] = Math.tanh(L[i] * 1.4) * fadeIn * fadeOut;
    R[i] = Math.tanh(R[i] * 1.4) * fadeIn * fadeOut;
    peak = Math.max(peak, Math.abs(L[i]), Math.abs(R[i]));
  }
  const norm = 0.84 / Math.max(peak, 1e-6);
  const buf = Buffer.alloc(44 + N * 4);
  buf.write("RIFF", 0); buf.writeUInt32LE(36 + N * 4, 4); buf.write("WAVE", 8);
  buf.write("fmt ", 12); buf.writeUInt32LE(16, 16); buf.writeUInt16LE(1, 20);
  buf.writeUInt16LE(2, 22); buf.writeUInt32LE(SR, 24); buf.writeUInt32LE(SR * 4, 28);
  buf.writeUInt16LE(4, 32); buf.writeUInt16LE(16, 34);
  buf.write("data", 36); buf.writeUInt32LE(N * 4, 40);
  for (let i = 0; i < N; i++) {
    buf.writeInt16LE(Math.max(-1, Math.min(1, L[i] * norm)) * 32767 | 0, 44 + i * 4);
    buf.writeInt16LE(Math.max(-1, Math.min(1, R[i] * norm)) * 32767 | 0, 44 + i * 4 + 2);
  }
  fs.writeFileSync(OUT, buf);
  console.log(`score2.wav written: ${(buf.length / 1e6).toFixed(1)} MB`);
}
