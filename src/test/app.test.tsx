import { beforeEach, describe, expect, it, vi } from "vitest";
import { StrictMode } from "react";
import { api } from "@/api";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import App from "@/App";
import { backend } from "./fakeBackend";
import { useJobStore } from "@/stores/jobStore";
import { useLearningStore } from "@/stores/learningStore";
import { useLessonStore } from "@/stores/lessonStore";
import { useRouteStore } from "@/stores/routeStore";
import { useSourceStore } from "@/stores/sourceStore";
import { useNavStore } from "@/stores/uiStore";
import { resetSwipeEngine } from "@/lib/swipeEngine";

function mountApp() {
  document.body.innerHTML = '<div id="root"></div><div id="toast" role="status"></div>';
  const container = document.getElementById("root") as HTMLElement;
  return render(<App />, { container });
}

/** The header title is the route switcher's label — the app's current route. */
const subjectName = () => document.querySelector(".subject-name")?.textContent ?? null;

async function boot() {
  await waitFor(() => expect(subjectName()).toBe("射影几何"));
}

/** jsdom has no PointerEvent, so drive the gesture with MouseEvents of the same type. */
function swipe(element: HTMLElement, from: number, to: number) {
  const at = (type: string, clientX: number) =>
    new MouseEvent(type, {
      bubbles: true,
      cancelable: true,
      clientX,
      clientY: 400,
      button: 0,
    });
  act(() => {
    element.dispatchEvent(at("pointerdown", from));
    element.dispatchEvent(at("pointermove", (from + to) / 2));
    element.dispatchEvent(at("pointermove", to));
    element.dispatchEvent(at("pointerup", to));
  });
}

async function clickAsync(element: Element | null) {
  expect(element).toBeTruthy();
  await act(async () => {
    fireEvent.click(element as Element);
    // The press/release feedback defers navigation by ~52ms so the release
    // animation is visible; give it time before asserting.
    await new Promise((resolve) => window.setTimeout(resolve, 120));
  });
}

/** Every store is a module singleton, so tests must start from a cold app. */
function resetStores() {
  // The gesture engine is a module singleton: a swipe in an earlier test would
  // otherwise keep suppressing clicks for another 90ms.
  resetSwipeEngine();
  useNavStore.setState({
    view: "home",
    tab: "todo",
    activeNodeId: null,
    direction: 1,
  });
  useRouteStore.setState({
    routes: [],
    activeRouteId: null,
    details: {},
    loading: false,
    error: null,
    scrollPositions: {},
    adaptations: {},
    noticeClosed: {},
  });
  useLearningStore.getState().reset();
  useLessonStore.getState().invalidate();
  useJobStore.getState().reset();
  useSourceStore.setState({ sources: [], recent: [], loading: false, error: null });
}

beforeEach(() => {
  resetStores();
  backend.routes.length = 0;
  backend.nodes.clear();
  backend.questions.clear();
  backend.adaptations.clear();
  backend.jobs.clear();
  backend.sessions.clear();
  backend.sources.length = 0;
  backend.calls.length = 0;
  // Re-seed exactly like `bootstrap.rs` does on a fresh install.
  backend.seed("射影几何", "geometry");
  backend.seed("线性代数", "algebra");
});

describe("学涯 shell", () => {
  it("opens an empty workspace and lets a first-time user create a route", async () => {
    backend.routes.length = 0;
    mountApp();
    await screen.findByRole("heading", { name: "从第一条学习路线开始" });
    expect(screen.queryByText("正在打开学习空间")).toBeNull();
    await clickAsync(screen.getByRole("button", { name: "创建学习路线" }));
    await waitFor(() => expect(document.querySelector(".route-setup")).toBeTruthy());
  });

  it("shows a recoverable error instead of an endless spinner when route loading fails", async () => {
    const request = vi.spyOn(api, "routeList").mockRejectedValue({ code: "IO_ERROR", message: "读取失败" });
    try {
      mountApp();
      expect(await screen.findByRole("alert")).toHaveTextContent("读取失败");
      expect(screen.queryByText("正在打开学习空间")).toBeNull();
    } finally {
      request.mockRestore();
    }
  });

  it("finishes bootstrap under StrictMode", async () => {
    render(<StrictMode><App /></StrictMode>);
    await boot();
  });

  it("boots into the first route and lists its roadmap nodes", async () => {
    mountApp();
    await boot();
    expect(document.querySelectorAll(".stop").length).toBe(6);
    expect(document.querySelector(".stop .node.done")).toBeTruthy();
    expect(document.querySelector(".stop .node.current")).toBeTruthy();
  });

  it("switches routes with a weighted horizontal swipe", async () => {
    mountApp();
    await boot();
    const zone = document.getElementById("screen") as HTMLElement;

    swipe(zone, 320, 90);

    await waitFor(() => expect(subjectName()).toBe("线性代数"));
    expect((document.querySelector(".home-page") as HTMLElement).style.transform).toBe("");
    // The swipe marked the incoming roadmap as settled, so entrance animations
    // do not replay on top of the slide.
    expect(document.querySelector(".home-page")?.className).toContain("swipe-settled");
  });

  it("ignores a swipe that does not pass the commit distance", async () => {
    mountApp();
    await boot();
    const zone = document.getElementById("screen") as HTMLElement;

    swipe(zone, 320, 300);

    await new Promise((resolve) => window.setTimeout(resolve, 60));
    expect(subjectName()).toBe("射影几何");
  });
});

describe("lesson", () => {
  it("opens a node as a full-screen lesson and shows its prepared plan", async () => {
    mountApp();
    await boot();

    const hit = document.querySelector('.stop[data-node-id="route-geometry-node-1"] .node-hit');
    await clickAsync(hit);

    await waitFor(() =>
      expect(document.querySelector(".page-title")?.textContent).toBe("无穷远点"),
    );
    await waitFor(() => expect(document.querySelectorAll(".todo").length).toBe(4));
    expect(backend.calls).toContain("course_node_prepare");
    // The underlay that froze the roadmap is always cleaned up.
    await waitFor(() => expect(document.querySelector(".lesson-underlay")).toBeNull());
  });

  it("returns to the roadmap when the back button is used", async () => {
    mountApp();
    await boot();
    await clickAsync(document.querySelector('.stop[data-node-id="route-geometry-node-1"] .node-hit'));
    await waitFor(() => expect(document.querySelector(".page-title")).toBeTruthy());

    await clickAsync(document.querySelector(".back"));
    await waitFor(() => expect(document.querySelector(".home-page")).toBeTruthy());
    expect(document.querySelector(".lesson-opening")).toBeNull();
  });
});

describe("learning loop", () => {
  async function enterLearning(openAssessment = true) {
    await boot();
    await clickAsync(document.querySelector('.stop[data-node-id="route-geometry-node-1"] .node-hit'));
    await waitFor(() => expect(document.querySelectorAll(".todo").length).toBe(4));
    await clickAsync(document.querySelector(".lesson-dock .primary"));
    // The agent must open the conversation before the learner does anything.
    await waitFor(() => expect(document.querySelector(".message.assistant")).toBeTruthy(), {
      timeout: 3000,
    });
    if (openAssessment) {
      await clickAsync(screen.getByRole("button", { name: /阶段检查/ }));
      await waitFor(() => expect(document.querySelector(".quiz-card")).toBeTruthy(), {
        timeout: 3000,
      });
    }
  }
  it("starts with an agent-authored opening and keeps the conversation primary", async () => {
    mountApp();
    await enterLearning(false);

    expect(backend.calls).toContain("learning_start");
    expect(document.body.textContent).toContain("先不急着记定义");
    expect(document.querySelector(".quiz-card")).toBeNull();

    const input = document.querySelector("#chat-input") as HTMLInputElement;
    await act(async () => {
      fireEvent.change(input, { target: { value: "我觉得是因为方向相同" } });
    });
    await clickAsync(screen.getByRole("button", { name: "发送消息" }));
    await waitFor(() => expect(backend.calls).toContain("tutor_send"));
    await waitFor(() =>
      expect(document.body.textContent).toContain("收到。我会根据你的回答继续提问。"),
    );
  });


  it("starts a session, grades a single choice answer and lights up the plan", async () => {
    mountApp();
    await enterLearning();

    expect(backend.calls).toContain("learning_start");
    const firstStep = document.querySelector(".plan-step");
    expect(firstStep?.className).toContain("is-current");

    await clickAsync(screen.getByText("无穷远点的正确说法"));
    await clickAsync(screen.getByText("提交答案"));

    await waitFor(() => expect(document.querySelector(".feedback")?.textContent).toContain("答对了"));
    expect(backend.calls).toContain("quiz_submit");

    await clickAsync(screen.getByText("继续学习"));
    await waitFor(() => expect(document.querySelector(".quiz-tag")?.textContent).toContain("多选"));
    await waitFor(() =>
      expect(document.querySelectorAll(".plan-step")[0]?.className).toContain("is-done"),
    );
  });

  it("keeps a wrong answer retryable and records a review node on the roadmap", async () => {
    mountApp();
    await enterLearning();

    await clickAsync(screen.getByText("无穷远点的常见误解"));
    await clickAsync(screen.getByText("提交答案"));

    await waitFor(() => expect(document.querySelector(".feedback")?.textContent).toContain("再想一想"));
    // Wrong objective answers do not advance the plan.
    expect(document.querySelector(".quiz-tag")?.textContent).toContain("单选");

    const route = backend.routes.find((entry) => entry.id === "route-geometry");
    await waitFor(() =>
      expect(backend.nodes.get("route-geometry")?.some((node) => node.kind === "review" && node.generatedBy === "adaptation")).toBe(true),
    );
    expect(route).toBeTruthy();

    // Changing the answer clears the verdict so the learner can try again.
    await clickAsync(screen.getByText("无穷远点的正确说法"));
    expect(document.querySelector(".feedback")).toBeNull();
  });

  it("submits an ungraded short answer and can finish the lesson", async () => {
    mountApp();
    await enterLearning();

    // Walk the first three questions with correct answers.
    await clickAsync(screen.getByText("无穷远点的正确说法"));
    await clickAsync(screen.getByText("提交答案"));
    await waitFor(() => expect(document.querySelector(".feedback")).toBeTruthy());
    await clickAsync(screen.getByText("继续学习"));

    await clickAsync(screen.getByText("无穷远点的正确说法"));
    await clickAsync(screen.getByText("无穷远点与它自身一致"));
    await clickAsync(screen.getByText("提交答案"));
    await waitFor(() => expect(document.querySelector(".feedback")?.textContent).toContain("答对了"));
    await clickAsync(screen.getByText("继续学习"));

    const fill = document.querySelector("#written-answer") as HTMLInputElement;
    expect(fill).toBeTruthy();
    await act(async () => {
      fireEvent.change(fill, { target: { value: "无穷远点" } });
    });
    await clickAsync(screen.getByText("提交答案"));
    await waitFor(() => expect(document.querySelector(".feedback")?.textContent).toContain("答对了"));
    await clickAsync(screen.getByText("继续学习"));

    // Short answer: not locally gradable, so it is submitted and can advance.
    const textarea = document.querySelector("#written-answer") as HTMLTextAreaElement;
    await act(async () => {
      fireEvent.change(textarea, { target: { value: "因为方向相同就对应同一个无穷远点" } });
    });
    await clickAsync(screen.getByText("提交答案"));
    await waitFor(() => expect(document.querySelector(".feedback")?.textContent).toContain("作答已提交"));
    await clickAsync(screen.getByText("继续学习"));

    // Photo answer: no image, so the button stays disabled until one is chosen.
    const submit = screen.getByText("提交答案").closest("button") as HTMLButtonElement;
    expect(submit.disabled).toBe(true);
  });

  it("asks the model to grade a free-text answer and shows the verdict", async () => {
    mountApp();
    await enterLearning();

    // Walk the locally gradable questions so the short answer comes up.
    await clickAsync(screen.getByText("无穷远点的正确说法"));
    await clickAsync(screen.getByText("提交答案"));
    await waitFor(() => expect(document.querySelector(".feedback")).toBeTruthy());
    await clickAsync(screen.getByText("继续学习"));

    await clickAsync(screen.getByText("无穷远点的正确说法"));
    await clickAsync(screen.getByText("无穷远点与它自身一致"));
    await clickAsync(screen.getByText("提交答案"));
    await waitFor(() => expect(document.querySelector(".feedback")?.textContent).toContain("答对了"));
    await clickAsync(screen.getByText("继续学习"));

    const fill = document.querySelector("#written-answer") as HTMLInputElement;
    await act(async () => {
      fireEvent.change(fill, { target: { value: "无穷远点" } });
    });
    await clickAsync(screen.getByText("提交答案"));
    await waitFor(() => expect(document.querySelector(".feedback")?.textContent).toContain("答对了"));
    await clickAsync(screen.getByText("继续学习"));

    const textarea = document.querySelector("#written-answer") as HTMLTextAreaElement;
    await act(async () => {
      fireEvent.change(textarea, { target: { value: "因为方向相同就对应同一个无穷远点" } });
    });
    await clickAsync(screen.getByText("提交答案"));
    await waitFor(() => expect(document.querySelector(".feedback")?.textContent).toContain("作答已提交"));

    // Objective answers are graded in Rust; free text has to be asked for.
    await clickAsync(screen.getByText("让 学涯 评阅这段作答"));
    await waitFor(() => expect(backend.calls).toContain("learning_respond"));
    await waitFor(() =>
      expect(document.querySelector(".feedback")?.textContent).toContain("已评阅 · 正确度 62%"),
    );
    expect(document.body.textContent).toContain("把无穷远点当成了普通点");
    // The verdict is part of the conversation, not a disappearing toast.
    expect(document.body.textContent).toContain(
      "你抓住了方向对应的直觉，但还没有说清它为什么必然是「一个」点。",
    );
  });
});

describe("route creation", () => {
  it("creates a route from the management layer and lands on its roadmap", async () => {
    mountApp();
    await boot();

    await clickAsync(document.querySelector(".subject-switch"));
    await waitFor(() => expect(document.querySelector(".roadmap-layer")).toBeTruthy());

    await clickAsync(document.querySelector(".add-unknown"));
    await waitFor(() => expect(document.querySelector("#setup-topic")).toBeTruthy());

    const topic = document.querySelector("#setup-topic") as HTMLInputElement;
    await act(async () => {
      fireEvent.change(topic, { target: { value: "伽罗瓦理论" } });
    });

    for (let step = 0; step < 3; step++) {
      await clickAsync(document.querySelector(".setup-actions .primary"));
    }

    await waitFor(() => expect(backend.calls).toContain("course_create"), { timeout: 3000 });
    await waitFor(() => expect(document.querySelector(".generation-page")).toBeTruthy());
    // The job reports completion on the first poll, so the app returns home.
    await waitFor(() => expect(subjectName()).toBe("伽罗瓦理论"), { timeout: 4000 });
    expect(backend.calls).toContain("job_get");
  });
});

describe("management layer", () => {
  async function openLayer() {
    await clickAsync(document.querySelector(".subject-switch"));
    await waitFor(() => expect(document.querySelector(".roadmap-layer")).toBeTruthy());
  }

  function layerButton(label: string): HTMLButtonElement {
    const found = Array.from(
      document.querySelectorAll<HTMLButtonElement>(".roadmap-layer button"),
    ).find((button) => button.textContent?.includes(label));
    expect(found, `layer button "${label}"`).toBeTruthy();
    return found as HTMLButtonElement;
  }

  it("saves a BYOK provider and never echoes the key back to the UI", async () => {
    mountApp();
    await boot();
    await openLayer();

    await clickAsync(layerButton("设置"));
    await waitFor(() => expect(document.querySelector("#provider-form")).toBeTruthy());

    const set = (selector: string, value: string) => {
      const field = document.querySelector(selector) as HTMLInputElement;
      expect(field).toBeTruthy();
      fireEvent.change(field, { target: { value } });
    };
    await act(async () => {
      set("#provider-name", "我的 OpenAI");
      set("#provider-base", "https://api.example.com/v1");
      set("#provider-chat", "gpt-4o-mini");
      set("#provider-key", "sk-secret-value");
    });

    await act(async () => {
      fireEvent.submit(document.querySelector("#provider-form") as HTMLFormElement);
      await new Promise((resolve) => window.setTimeout(resolve, 60));
    });

    await waitFor(() => expect(backend.providers.length).toBe(1));
    expect(backend.calls).toContain("provider_upsert");
    // Rust receives the key...
    expect(backend.providers[0].apiKey).toBe("sk-secret-value");
    // ...and the WebView is never given it back.
    await waitFor(() =>
      expect(document.body.textContent).toContain("API Key 已存入系统钥匙串"),
    );
    expect(document.body.textContent).not.toContain("sk-secret-value");
    expect((document.querySelector("#provider-key") as HTMLInputElement).value).toBe("");
  });

  it("lets the tutor change the roadmap and says so in the chat", async () => {
    mountApp();
    await boot();
    await clickAsync(document.querySelector(".node-hit"));
    await waitFor(() => expect(document.querySelector(".lesson-dock .primary")).toBeTruthy());
    await clickAsync(document.querySelector(".lesson-dock .primary"));
    await waitFor(() => expect(document.querySelector("#chat-input")).toBeTruthy());

    const nodesBefore = backend.nodeCount();
    await act(async () => {
      fireEvent.change(document.querySelector("#chat-input") as HTMLInputElement, {
        target: { value: "我还是不懂这里为什么要这样定义" },
      });
    });
    await act(async () => {
      fireEvent.submit(document.querySelector(".composer") as HTMLFormElement);
      await new Promise((resolve) => window.setTimeout(resolve, 80));
    });

    await waitFor(() => expect(backend.calls).toContain("tutor_send"));
    // The learner is told the roadmap moved, instead of finding out later.
    await waitFor(() =>
      expect(document.body.textContent).toContain("已在你的路线里插入一节针对性加练"),
    );
    expect(backend.nodeCount()).toBe(nodesBefore + 1);
  });

  it("collects web results as real sources before planning a route", async () => {
    // A search provider is configured, so the create form's toggle can deliver.
    backend.settings.braveApiKey = "brave-test-key";
    mountApp();
    await boot();
    await openLayer();

    await clickAsync(layerButton("探索新的未知"));
    await waitFor(() => expect(document.querySelector("#setup-topic")).toBeTruthy());
    await act(async () => {
      fireEvent.change(document.querySelector("#setup-topic") as HTMLInputElement, {
        target: { value: "伽罗瓦理论" },
      });
    });

    for (let step = 0; step < 2; step++) {
      await clickAsync(document.querySelector(".setup-actions .primary"));
    }

    await clickAsync(screen.getByRole("button", { name: "帮我找资料" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "生成我的路线" })).toBeEnabled(), { timeout: 3000 });
    await clickAsync(screen.getByRole("button", { name: "生成我的路线" }));

    await waitFor(() => expect(backend.calls).toContain("search_web"), { timeout: 3000 });
    await waitFor(() => expect(backend.calls).toContain("course_create"), {
      timeout: 3000,
    });
    // Both hits were imported, and their ids reached the planner.
    expect(backend.sources.length).toBe(2);
    // Import jobs are queued first, so pick the planning job by its payload.
    const planning = [...backend.jobs.values()].find(
      (candidate) => (candidate as { __sourceIds?: string[] }).__sourceIds,
    ) as { __sourceIds: string[] };
    expect(planning.__sourceIds).toHaveLength(2);
    await waitFor(() => expect(subjectName()).toBe("伽罗瓦理论"), { timeout: 4000 });
  });

  it("lists imported sources and can drop one", async () => {
    backend.sources.push({
      id: "source-1",
      kind: "file",
      title: "线性代数讲义",
      uri: null,
      originalFilename: "linear-algebra.pdf",
      status: "ready",
      contentHash: null,
      createdAt: "2026-01-01T00:00:00Z",
    });
    mountApp();
    await boot();
    await openLayer();

    await clickAsync(layerButton("资料库"));
    await waitFor(() => expect(screen.getByText("线性代数讲义")).toBeTruthy());
    expect(document.body.textContent).toContain("已索引，可被引用");

    await clickAsync(
      document.querySelector('button[aria-label="移除线性代数讲义"]'),
    );
    await waitFor(() => expect(backend.calls).toContain("source_delete"));
  });
});
