/**
 * ORBIT promo — product UI capture.
 *
 * Drives the real frontend via preview.html?fixtures=1 (in-memory IPC backend,
 * no Rust). The product repo is read-only: this script only navigates and
 * patches window.__TAURI_INTERNALS__.invoke at runtime (same mechanism the
 * preview entry itself uses) to script a richer tutor conversation for the
 * camera. Nothing is written to the product.
 *
 * Usage: node capture/capture.mjs [filter]
 * Output: promo-film/shots/*.png  (3840×2160 masters)
 */
import { chromium } from "playwright";
import { fileURLToPath } from "node:url";
import path from "node:path";
import fs from "node:fs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SHOTS = path.join(ROOT, "shots");
fs.mkdirSync(SHOTS, { recursive: true });

const BASE = "http://localhost:1420/preview.html?fixtures=1";
const NOW = "2026-01-01T00:00:00Z";

/** Rich mock IPC: extends the preview fixtures with a scripted tutor arc. */
const MOCK_PATCH = `
(() => {
  const NOW = "${NOW}";
  const tutorTurns = [
    {
      message: "很自然的猜测——但注意你刚才用的词：『保持不变』。长度在投影下其实是会变的。\\n\\n换个问法：如果长度会变，四个点之间还有什么关系，是投影拿不走的？",
      suggestedAction: "probe",
      diagnosis: null, diagnosisDetail: null,
      citations: [], grounding: "conversational", generatedBy: "atlas", routeUpdated: false,
      insertedNodeId: null, adaptation: null,
    },
    {
      message: "对。交比 (AB,CD) 在任意射影变换下保持不变——所以它才叫『射影不变量』。\\n\\n往下走一步：当 (AB,CD) = −1，这四点构成调和分割。你能在身边找一个它的影子吗？",
      suggestedAction: "challenge",
      diagnosis: null, diagnosisDetail: null,
      citations: [{
        chunkId: "c1", sourceId: "s1", sourceTitle: "《高等几何》讲义 · 第三章",
        sourceKind: "file", sourceUri: null, sourceFilename: "projective-geometry.pdf",
        text: "交比在射影变换下保持不变；调和分割即交比为 −1 的特例。",
      }],
      grounding: "grounded", generatedBy: "atlas", routeUpdated: false,
      insertedNodeId: null, adaptation: null,
    },
  ];
  let tutorIndex = 0;
  const jobStarted = Date.now();
  const baseNode = (id, title, orderHint, status, kind, mins) => ({
    id, courseId: "r1", title, description: "", objective: "", orderHint,
    estimatedMinutes: mins, status, kind, confidence: 1, reviewAt: null,
    generatedBy: "seed", metadataJson: "{}",
  });
  window.__ORBIT_PREV_INVOKE__ = window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {
    switch (cmd) {
      case "learning_start":
        return {
          sessionId: "sess-promo",
          lesson: await window.__ORBIT_PREV_INVOKE__("course_node_prepare", { nodeId: args?.input?.nodeId ?? args?.nodeId }),
          mastery: 0.4,
          openingTurn: {
            message: "我们从一个直觉开始。\\n\\n想象一条笔直的铁轨：四条平行的枕木，被你的视线投影到视网膜上。四个共线点 A、B、C、D，经过一次投影——\\n\\n你觉得，有什么会保持不变？",
            suggestedAction: "ask",
            diagnosis: null, diagnosisDetail: null, citations: [],
            grounding: "conversational", generatedBy: "atlas", routeUpdated: false,
            insertedNodeId: null, adaptation: null,
          },
          openingError: null,
        };
      case "tutor_send_stream":
      case "tutor_send": {
        const turn = tutorTurns[Math.min(tutorIndex++, tutorTurns.length - 1)];
        await new Promise((r) => setTimeout(r, 900));
        return turn;
      }
      case "quiz_submit":
        await new Promise((r) => setTimeout(r, 700));
        return {
          questionId: args?.input?.questionId ?? "q1", nodeId: "n3",
          correct: false, graded: true, score: 0.5,
          feedback: "这是距离之比，不是交比。交比分母上是 AD——注意四个点出现的顺序。",
          expected: ["(AC·BD)/(BC·AD)"], misconceptions: ["把交比误记为距离之比"],
          mastery: 0.52, confidence: 0.6, nextReview: null,
          adaptation: {
            id: "ad1", courseId: "r1", nodeId: "n3", kind: "insert_review",
            reason: "检测到误解：交比的定义顺序", payloadJson: "{}", createdAt: NOW,
          },
          insertedNodeId: "n3r",
        };
      case "learning_respond":
        return {
          feedback: "方向对了一半。", correctness: 0.6, misconceptions: [],
          masteryDelta: 0.08, followUpQuestion: null, suggestedAction: "continue",
          mastery: 0.6, attempts: 1, verdict: null, strengths: [], gaps: [],
          diagnoses: [], confidence: 0.7, adaptation: null, insertedNodeId: null,
        };
      case "learning_finish": return null;
      case "course_node_complete":
        return { course: null, completedNodeId: args?.nodeId, nextNodeId: "n4", adaptation: null };
      case "course_create":
        return { id: "job-promo", kind: "course_create", status: "running", progress: 0,
          message: "", errorJson: null, resultJson: null, createdAt: NOW, startedAt: NOW, completedAt: null };
      case "job_get": {
        const p = Math.min(1, (Date.now() - jobStarted) / 9000);
        return { id: "job-promo", kind: "course_create", status: p >= 1 ? "completed" : "running",
          progress: p, message: "", errorJson: null,
          resultJson: p >= 1 ? JSON.stringify({ courseId: "r2" }) : null,
          createdAt: NOW, startedAt: NOW, completedAt: p >= 1 ? NOW : null };
      }
      case "plugin:event|listen": return 0;
      case "plugin:event|unlisten": return null;
      default:
        return window.__ORBIT_PREV_INVOKE__(cmd, args);
    }
  };
})();
`;

const themes = {
  lightBuoy: { visualMotif: "buoy", iconMotif: "buoy", colorScheme: "light" },
  darkLighthouse: { visualMotif: "lighthouse", iconMotif: "lighthouse", colorScheme: "dark" },
  lightLighthouse: { visualMotif: "lighthouse", iconMotif: "lighthouse", colorScheme: "light" },
};

const filter = process.argv[2] ?? "";

async function shoot(page, name) {
  if (filter && !name.includes(filter)) return false;
  await page.screenshot({ path: path.join(SHOTS, `${name}.png`) });
  console.log("shot:", name);
  return true;
}

function themeInit(theme) {
  return (
    `localStorage.setItem("verruckt.theme.v2", ${JSON.stringify(JSON.stringify(theme))});` +
    `localStorage.setItem("verruckt.localStart", "1");`
  );
}

async function main() {
  const browser = await chromium.launch();
  const ctx = await browser.newContext({
    viewport: { width: 1920, height: 1080 },
    deviceScaleFactor: 2,
    reducedMotion: "no-preference",
  });
  const page = await ctx.newPage();
  page.on("pageerror", (e) => console.warn("pageerror:", e.message));

  /* ---- 0. onboarding gate (brand copy) ---- */
  await page.goto(BASE, { waitUntil: "networkidle" });
  await page.waitForTimeout(2600);
  await shoot(page, "00-onboarding");

  /* ---- 1. roadmap, light + buoy ---- */
  await page.addInitScript(themeInit(themes.lightBuoy));
  await page.goto(BASE, { waitUntil: "networkidle" });
  await page.waitForSelector(".stop", { timeout: 15000 });
  await page.waitForTimeout(2600);
  await shoot(page, "01-roadmap-light-buoy");

  /* ---- 2. roadmap, dark + lighthouse ---- */
  await page.addInitScript(themeInit(themes.darkLighthouse));
  await page.goto(BASE, { waitUntil: "networkidle" });
  await page.waitForSelector(".stop", { timeout: 15000 });
  await page.waitForTimeout(2600);
  await shoot(page, "02-roadmap-dark-lighthouse");

  /* ---- 3. lesson view, light ---- */
  await page.addInitScript(themeInit(themes.lightBuoy));
  await page.goto(`${BASE}&view=lesson`, { waitUntil: "networkidle" });
  await page.waitForTimeout(2400);
  await shoot(page, "03-lesson-light");

  /* ---- 4. learning conversation, light — scripted via runtime mock ---- */
  await page.addInitScript(themeInit(themes.lightBuoy));
  await page.goto(BASE, { waitUntil: "networkidle" });
  await page.waitForSelector(".stop", { timeout: 15000 });
  await page.evaluate(MOCK_PATCH);
  await page.click('.stop[data-node-id="n3"] .stop-info');
  await page.waitForTimeout(1800);
  await page.click("button.primary.floating");
  await page.waitForSelector(".chat-page", { timeout: 15000 });
  await page.waitForTimeout(2200);
  await shoot(page, "04-learning-opening-light");

  // learner answers (wrong intuition), tutor probes
  await page.fill("#chat-input", "长度保持不变？");
  await page.press("#chat-input", "Enter");
  await page.waitForTimeout(2600);
  // learner answers again, tutor confirms + challenges, with citation
  await page.fill("#chat-input", "交比？四个点的交比不变。");
  await page.press("#chat-input", "Enter");
  await page.waitForTimeout(3000);
  await shoot(page, "05-learning-conversation-light");

  // quiz panel
  try {
    await page.click('button.secondary:has-text("阶段检查")', { timeout: 4000 });
    await page.waitForTimeout(1200);
    await shoot(page, "06-learning-quiz-light");
    const option = page.locator("#quiz .option, #quiz button").first();
    await option.click({ timeout: 3000 });
    await page.waitForTimeout(500);
    const submit = page.getByRole("button", { name: /提交|确认|检查/ }).first();
    await submit.click({ timeout: 3000 });
    await page.waitForTimeout(2200);
    await shoot(page, "07-learning-quiz-feedback-light");
  } catch (e) {
    console.warn("quiz capture skipped:", e.message);
  }

  /* ---- 5. learning conversation, dark lighthouse ---- */
  await page.addInitScript(themeInit(themes.darkLighthouse));
  await page.goto(BASE, { waitUntil: "networkidle" });
  await page.waitForSelector(".stop", { timeout: 15000 });
  await page.evaluate(MOCK_PATCH);
  await page.click('.stop[data-node-id="n3"] .stop-info');
  await page.waitForTimeout(1600);
  await shoot(page, "08-lesson-dark-lighthouse");
  await page.click("button.primary.floating");
  await page.waitForSelector(".chat-page", { timeout: 15000 });
  await page.waitForTimeout(2200);
  await page.fill("#chat-input", "长度保持不变？");
  await page.press("#chat-input", "Enter");
  await page.waitForTimeout(2600);
  await page.fill("#chat-input", "交比？四个点的交比不变。");
  await page.press("#chat-input", "Enter");
  await page.waitForTimeout(3000);
  await shoot(page, "09-learning-conversation-dark");

  /* ---- 6. create + generating ---- */
  await page.addInitScript(themeInit(themes.lightBuoy));
  await page.goto(`${BASE}&view=create`, { waitUntil: "networkidle" });
  await page.waitForTimeout(2000);
  await shoot(page, "10-create-light");

  await page.evaluate(MOCK_PATCH);
  try {
    await page.fill("#setup-topic", "潮汐与月球");
    for (let i = 0; i < 3; i++) {
      if (await page.locator(".generation-page").count()) break;
      const btn = page.locator(".setup-actions button.primary").first();
      await btn.click({ timeout: 4000 });
      await page.waitForTimeout(1500);
    }
    await page.waitForTimeout(4200);
    await shoot(page, "11-generating-light");
  } catch (e) {
    console.warn("generating capture skipped:", e.message);
  }

  /* ---- 7. route layer (archive drawer) ---- */
  await page.addInitScript(themeInit(themes.darkLighthouse));
  await page.goto(BASE, { waitUntil: "networkidle" });
  await page.waitForSelector(".stop", { timeout: 15000 });
  await page.waitForTimeout(2000);
  try {
    await page.click(".app-header button", { timeout: 3000 });
    await page.waitForTimeout(1400);
    await shoot(page, "12-route-layer-dark");
  } catch (e) {
    console.warn("layer capture skipped:", e.message);
  }

  await browser.close();
  console.log("done.");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
