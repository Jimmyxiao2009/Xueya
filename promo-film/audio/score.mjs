/**
 * ORBIT promo — procedural score.
 * 60s stereo 48kHz 16-bit WAV, no samples, no licensing surface.
 *
 * Layers (times in seconds, mirrors STORYBOARD.md):
 *   wind/sea bed      0–60      brown noise, slow LP, breathing amplitude
 *   sonar pings       1.15 / 19.2 / 36.6 / 52.05   780Hz decay + echoes
 *   low pulse         heartbeat, tempo follows editing density
 *   paper rustles     S2 card entrances (5.55 + i*0.72)
 *   scan sweep        11.15–13.0 bandpass noise gliss + 3 ticks
 *   pad               13.2 → 52  detuned triangles, D–A–E open chord
 *   ticks             route connections, pedagogy tags, mastery
 *   the drop          38.6–41.2 near silence; single wooden click at 41.0
 *   brand             52.05 ping; 53.2–55.8 draw shimmer; 56.0 resolve chord
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SR = 48000, DUR = 60, N = SR * DUR;
const OUT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "score.wav");

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

/** one-pole lowpass state */
function lp(cutoff) {
  const rc = 1 / (2 * Math.PI * cutoff), dt = 1 / SR;
  const a = dt / (rc + dt);
  let y = 0;
  return (x) => (y += a * (x - y));
}
function hp(cutoff) {
  const rc = 1 / (2 * Math.PI * cutoff), dt = 1 / SR;
  const a = rc / (rc + dt);
  let y = 0, xPrev = 0;
  return (x) => { const v = a * (y + x - xPrev); y = v; xPrev = x; return v; };
}

function addAt(t0, fn, len, gain, pan = 0) {
  const start = Math.floor(t0 * SR);
  const count = Math.min(Math.floor(len * SR), N - start);
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
  const lpSlow = lp(320), lpSlowR = lp(280);
  let last = 0, lastR = 0;
  for (let i = 0; i < N; i++) {
    const t = i / SR;
    const w = rng() * 2 - 1, wR = rng() * 2 - 1;
    last = (last + 0.02 * w) / 1.02;         // brown-ish
    lastR = (lastR + 0.02 * wR) / 1.02;
    const breathe = 0.55 + 0.45 * Math.sin(t * 0.42 + 1.3) * Math.sin(t * 0.11);
    // duck under the thesis drop and the brand silence
    const drop = 1 - 0.92 * smooth(seg(t, 38.4, 39.2)) * (1 - smooth(seg(t, 41.4, 42.4)));
    const brand = 1 - 0.85 * smooth(seg(t, 51.6, 52.2)) * (1 - smooth(seg(t, 58.4, 59.6)));
    const g = 0.16 * breathe * drop * brand;
    L[i] += lpSlow(last) * g * 8;
    R[i] += lpSlowR(lastR) * g * 8;
  }
}

/* ----------------------------------------------------------------- pings */
function ping(t0, freq, gain, echoes, pan = 0) {
  for (let e = 0; e < echoes; e++) {
    const dt = e * 0.42, g = gain * Math.pow(0.45, e);
    addAt(t0 + dt, (t) => {
      const env = Math.exp(-t * 7.5);
      const vib = 1 + 0.002 * Math.sin(t * 30);
      return Math.sin(2 * Math.PI * freq * vib * t) * env * (t < 0.004 ? t / 0.004 : 1);
    }, 1.4, g, pan + e * 0.12);
    // faint upper harmonic
    addAt(t0 + dt, (t) => Math.sin(2 * Math.PI * freq * 2.01 * t) * Math.exp(-t * 11), 1.0, g * 0.25, pan);
  }
}
ping(1.15, 780, 0.5, 3, 0.25);       // LOST — the first signal
ping(19.2, 660, 0.34, 3, -0.1);      // MAP formed
ping(36.6, 880, 0.30, 2, 0.05);      // THINK — close, dry
ping(52.05, 780, 0.55, 2, 0);        // BRAND — driest

/* ----------------------------------------------------------------- pulse */
function thump(t0, gain, freq = 52) {
  addAt(t0, (t) => {
    const f = freq * Math.exp(-t * 6);
    return Math.sin(2 * Math.PI * f * t) * Math.exp(-t * 9);
  }, 0.5, gain);
  addAt(t0, (t) => (mulberry32(7)() * 2 - 1) * Math.exp(-t * 220), 0.03, gain * 0.5);
}
// LOST: 60bpm heartbeat
for (let k = 0; k < 4; k++) thump(2.0 + k * 1.0, 0.16);
// ARCHIVE: densify with the cut rate
for (let k = 0; k < 9; k++) thump(5.55 + k * 0.72, 0.15, 58);
// MAP→ORBIT: steady 76bpm
for (let t = 13.4; t < 31.6; t += 0.79) thump(t, 0.13);
// WORLD: widen
for (let t = 45.0; t < 51.6; t += 0.79) thump(t, 0.15, 56);

/* -------------------------------------------------------------- paper */
function rustle(t0, gain) {
  const rng = mulberry32(Math.floor(t0 * 1000));
  const hpF = hp(1400);
  addAt(t0, (t) => {
    const flap = 0.5 + 0.5 * Math.sin(t * 90 + rng() * 6);
    const env = Math.exp(-t * 16) * (t < 0.012 ? t / 0.012 : 1);
    return hpF((rng() * 2 - 1)) * env * flap;
  }, 0.35, gain, (rng() - 0.5) * 0.6);
}
for (let i = 0; i < 7; i++) rustle(5.55 + i * 0.72, 0.30);
rustle(26.6, 0.28); rustle(27.5, 0.28);            // occlusion wipes into product
for (let i = 0; i < 4; i++) rustle(42.2 + i * 0.5, 0.20);  // deconstruction lift-off

/* ------------------------------------------------------------- scan sweep */
{
  const rng = mulberry32(99);
  let phase = 0;
  const start = 11.15, end = 13.0;
  const s0 = Math.floor(start * SR), s1 = Math.floor(end * SR);
  for (let i = s0; i < s1; i++) {
    const t = i / SR, p = (t - start) / (end - start);
    const f = 380 + p * 2600;
    phase += 2 * Math.PI * f / SR;
    const tone = Math.sin(phase) * 0.05 * Math.sin(p * Math.PI);
    const noiseBand = (rng() * 2 - 1) * 0.03 * Math.sin(p * Math.PI);
    L[i] += tone + noiseBand; R[i] += tone * 0.9 + noiseBand;
  }
  // structure-found ticks as the beam crosses cards
  [11.5, 11.9, 12.35].forEach((tt) => tick(tt, 0.22, 2300));
}

/* ------------------------------------------------------------------ ticks */
function tick(t0, gain, freq = 1900) {
  addAt(t0, (t) => {
    const v = Math.sin(2 * Math.PI * freq * t) * Math.exp(-t * 90);
    return v + (mulberry32(3)() * 2 - 1) * Math.exp(-t * 300) * 0.6;
  }, 0.08, gain);
}
[16.5, 17.2, 17.9, 18.5].forEach((t) => tick(t, 0.16, 1500));          // route connects
[28.9, 29.45, 30.0, 30.55, 31.1].forEach((t) => tick(t, 0.15, 2000));  // pedagogy tags
tick(39.0, 0.18, 1200);                                                // mastery lights
tick(41.0, 0.26, 900);                                                 // the wooden click after silence

/* ------------------------------------------------------------------ pad */
function padNote(t0, t1, freq, gain, pan) {
  const start = Math.floor(t0 * SR), end = Math.floor(t1 * SR);
  const len = end - start;
  for (let i = 0; i < len; i++) {
    const t = i / SR, u = i / len;
    const env = smooth(clamp01(u / 0.25)) * (1 - smooth(seg(u, 0.7, 1)));
    const slow = 0.85 + 0.15 * Math.sin(t * 0.5 + freq);
    const v =
      Math.sin(2 * Math.PI * freq * t) * 0.5 +
      Math.sin(2 * Math.PI * (freq * 1.005) * t) * 0.3 +
      Math.sin(2 * Math.PI * (freq * 0.5) * t) * 0.2;
    const g = gain * env * slow;
    L[start + i] += v * g * Math.cos((pan + 1) * Math.PI / 4);
    R[start + i] += v * g * Math.sin((pan + 1) * Math.PI / 4);
  }
}
// D–A–E open chord, enters with the map
padNote(13.2, 21.5, 146.83, 0.055, -0.3);  // D3
padNote(13.6, 21.5, 220.0, 0.05, 0.2);     // A3
padNote(14.0, 21.5, 329.63, 0.035, 0.45);  // E4
// holds quietly through ORBIT+THINK, thinner
padNote(21.5, 31.8, 146.83, 0.04, -0.2);
padNote(21.5, 31.8, 220.0, 0.038, 0.25);
padNote(32.0, 38.4, 146.83, 0.03, -0.15);
padNote(32.0, 38.4, 293.66, 0.022, 0.3);   // D4 — sparse
// WORLD: opens up, add F# for lift
padNote(42.4, 51.8, 146.83, 0.05, -0.35);
padNote(42.4, 51.8, 220.0, 0.045, 0.1);
padNote(42.8, 51.8, 369.99, 0.03, 0.4);    // F#4
// resolve: warm D major add9 bloom at the wordmark
padNote(56.0, 60.0, 146.83, 0.06, -0.2);
padNote(56.1, 60.0, 220.0, 0.05, 0.15);
padNote(56.2, 60.0, 293.66, 0.04, 0.35);
padNote(56.3, 60.0, 369.99, 0.028, 0.5);

/* ---------------------------------------------------- brand draw shimmer */
{
  const s0 = Math.floor(53.2 * SR), s1 = Math.floor(55.9 * SR);
  let phase = 0;
  for (let i = s0; i < s1; i++) {
    const t = i / SR, p = (t - 53.2) / 2.7;
    const f = 2400 + p * 1800;
    phase += 2 * Math.PI * f / SR;
    const g = 0.022 * Math.sin(p * Math.PI);
    L[i] += Math.sin(phase) * g;
    R[i] += Math.sin(phase * 1.003) * g;
  }
}

/* ------------------------------------------------------- master + write */
{
  // gentle saturation/limit + fades
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
  console.log(`score.wav written: ${(buf.length / 1e6).toFixed(1)} MB, peak ${(20 * Math.log10(peak)).toFixed(1)} dBFS pre-norm`);
}
