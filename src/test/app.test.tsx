import { beforeEach, describe, expect, it } from "vitest";
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

describe("Orbit shell", () => {
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
  async function enterLearning() {
    await boot();
    await clickAsync(document.querySelector('.stop[data-node-id="route-geometry-node-1"] .node-hit'));
    await waitFor(() => expect(document.querySelectorAll(".todo").length).toBe(4));
    await clickAsync(document.querySelector(".lesson-dock .primary"));
    await waitFor(() => expect(document.querySelector(".quiz-card")).toBeTruthy(), {
      timeout: 3000,
    });
  }

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
});

describe("route creation", () => {
  it("creates a route from the management layer and lands on its roadmap", async () => {
    mountApp();
    await boot();

    await clickAsync(document.querySelector(".subject-switch"));
    await waitFor(() => expect(document.querySelector(".roadmap-layer")).toBeTruthy());

    await clickAsync(document.querySelector(".add-unknown"));
    await waitFor(() => expect(document.querySelector("#topic")).toBeTruthy());

    const topic = document.querySelector("#topic") as HTMLInputElement;
    await act(async () => {
      fireEvent.change(topic, { target: { value: "伽罗瓦理论" } });
    });

    await clickAsync(document.querySelector(".create-dock .primary"));

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
