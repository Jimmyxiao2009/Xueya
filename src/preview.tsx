/**
 * Visual-only preview entry. It installs an in-memory Tauri IPC backend BEFORE
 * the app module graph is imported, so the real screens render with real data
 * and real CSS in a plain browser (no Rust runtime). Used only by preview.html
 * for design review; never shipped to production builds.
 *
 * Command names and DTO shapes mirror src/api.ts exactly.
 */

import "@/styles/index.css";

const NOW = "2026-01-01T00:00:00Z";

const route = {
  id: "r1", topic: "射影几何", title: "射影几何",
  description: "从齐次坐标到配极变换", status: "active",
  goal: null, experienceLevel: null, desiredDepth: null, kind: "math",
  currentNodeId: "n3", seeded: true, nodeCount: 6, completedCount: 2,
  progress: 0.33, metadataJson: "{}", createdAt: NOW,
};

function node(id: string, title: string, orderHint: number, status: string, kind: string, estimatedMinutes: number) {
  return {
    id, courseId: "r1", title, description: "", objective: "", orderHint,
    estimatedMinutes, status, kind, confidence: 1, reviewAt: null,
    generatedBy: "seed", metadataJson: "{}",
  };
}

const nodes = [
  node("n1", "射影平面基础", 1, "completed", "lesson", 12),
  node("n2", "齐次坐标", 2, "completed", "lesson", 15),
  node("n3", "交比与调和分割", 3, "in_progress", "lesson", 18),
  node("n4", "对偶原理", 4, "pending", "review", 10),
  node("n5", "二次曲线", 5, "pending", "quiz", 14),
  node("n6", "配极变换", 6, "pending", "lesson", 16),
];

const lesson = {
  title: "交比与调和分割",
  explanation:
    "交比是射影几何中最核心的不变量。它描述四个共线点之间的相对位置关系，并且在任意射影变换下保持不变——这正是它有力量的原因。",
  keyPoints: ["交比在射影变换下保持不变", "四个共线点唯一确定一个交比", "调和分割是交比为 -1 的特例"],
  diagnosticQuestion: "四个共线点的交比在什么变换下保持不变？",
  citations: [],
  objectives: ["理解交比的定义", "会计算简单交比"],
  steps: [
    { id: "s1", ordinal: 1, title: "明确本课学习目标", detail: "先弄清楚这一课要解决什么问题。", kind: "read", durationLabel: "3 min" },
    { id: "s2", ordinal: 2, title: "阅读与整理资料", detail: "把资料里的说法和自己的理解对上。", kind: "read", durationLabel: "5 min" },
    { id: "s3", ordinal: 3, title: "形成自己的解释", detail: "用自己的话复述所学内容。", kind: "practice", durationLabel: "5 min" },
    { id: "s4", ordinal: 4, title: "通过练习检查掌握情况", detail: "答错会触发路线插入加练节点。", kind: "quiz", durationLabel: "4 min" },
  ],
  questions: [
    { id: "q1", nodeId: "n3", ordinal: 1, kind: "single", prompt: "四个共线点 A、B、C、D 的交比 (AB,CD) 定义为？", options: ["(AC·BD)/(BC·AD)", "(AB·CD)/(AC·BD)", "(AD·BC)/(AC·BD)"], difficulty: 2 },
  ],
  mastery: 0.4,
  estimatedMinutes: 18,
  nodeKind: "lesson",
  generatedBy: "builtin",
  hasEvidence: false,
};

async function fakeInvoke(command: string): Promise<unknown> {
  switch (command) {
    case "app_bootstrap":
    case "app_get_info":
      return { version: "0.1.0", dataDir: "", providerCount: 0, sourceCount: 0, courseCount: 1, searchConfigured: false };
    case "course_list": return [route];
    case "course_get": return { course: route, nodes, dependencies: [] };
    case "course_learner_states": return [];
    case "course_adaptations": return [];
    case "source_list": return [];
    case "settings_get": return { searchProvider: "", hasBraveApiKey: false, aiMode: "mock" };
    case "provider_list": return [];
    case "course_node_prepare": return lesson;
    case "course_node_get": return nodes[2];
    case "learning_start": return { sessionId: "sess1", lesson, mastery: 0.4 };
    case "quiz_list": return lesson.questions;
    case "job_list": return [];
    case "cloud_account_state": return { state: "signed_out" };
    case "sync_status": return { enabled: false };
    default: return null;
  }
}

const w = window as unknown as { __TAURI_INTERNALS__?: Record<string, unknown> };
w.__TAURI_INTERNALS__ = w.__TAURI_INTERNALS__ || {};
w.__TAURI_INTERNALS__.invoke = fakeInvoke;
w.__TAURI_INTERNALS__.transformCallback = () => 0;
w.__TAURI_INTERNALS__.convertFileSrc = (p: string) => p;
w.__TAURI_INTERNALS__.metadata = { currentWindow: { label: "main" }, currentWebview: { label: "main" } };

// Import the real app only after the backend is installed.
await import("@/main");

// Deep-link a screen for visual review: preview.html?view=lesson|learning|create.
// Waits for the app to boot, then drives the same store actions the UI uses.
const target = new URLSearchParams(window.location.search).get("view");
if (target && target !== "home") {
  const { useNavStore } = await import("@/stores/uiStore");
  const { useRouteStore } = await import("@/stores/routeStore");
  const { useLessonStore } = await import("@/stores/lessonStore");
  const { useLearningStore } = await import("@/stores/learningStore");
  // Poll until the route has loaded and the app has booted.
  const ready = await new Promise<boolean>((resolve) => {
    const t0 = Date.now();
    const tick = () => {
      const has = useRouteStore.getState().routes.length > 0;
      if (has) return resolve(true);
      if (Date.now() - t0 > 8000) return resolve(false);
      window.setTimeout(tick, 120);
    };
    tick();
  });
  if (ready) {
    const nav = useNavStore.getState();
    if (target === "lesson") {
      nav.setActiveNode("n3");
      await useLessonStore.getState().prepare("n3");
      nav.setView("lesson", 1);
    } else if (target === "learning") {
      nav.setActiveNode("n3");
      await useLessonStore.getState().prepare("n3");
      await useLearningStore.getState().start("n3");
      nav.setView("learning", 1);
    } else if (target === "create") {
      nav.setView("create", 1);
    }
  }
}
