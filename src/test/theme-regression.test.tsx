import { act, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { LearningRoute, Lesson, RouteNode } from "@/api";
import { LearningScreen } from "@/components/learning/LearningScreen";
import { LessonScreen } from "@/components/lesson/LessonScreen";
import { SettingsScreen } from "@/screens/SettingsScreen";
import { CreateRouteScreen } from "@/screens/CreateRouteScreen";
import { useLearningStore } from "@/stores/learningStore";
import { useLessonStore } from "@/stores/lessonStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useCloudStore } from "@/stores/cloudStore";
import { useThemeStore } from "@/stores/themeStore";

const route: LearningRoute = {
  id: "r",
  topic: "几何",
  title: "几何",
  description: "",
  status: "active",
  kind: "math",
  currentNodeId: "n1",
  seeded: false,
  nodeCount: 1,
  completedCount: 0,
  progress: 0,
  metadataJson: "{}",
  createdAt: "2026-01-01",
};

const node: RouteNode = {
  id: "n1",
  courseId: "r",
  title: "概念",
  description: "",
  objective: "理解概念",
  orderHint: 0,
  estimatedMinutes: 10,
  status: "available",
  kind: "lesson",
  confidence: 1,
  generatedBy: "test",
  metadataJson: "{}",
};

const lesson: Lesson = {
  title: "概念",
  explanation: "完整讲解不应该在预览中出现",
  keyPoints: ["关键点"],
  diagnosticQuestion: "为什么？",
  citations: [],
  steps: [{ id: "s1", ordinal: 0, title: "理解", detail: "建立直觉", kind: "concept", durationLabel: "5 min" }],
  questions: [],
  mastery: 0.2,
  estimatedMinutes: 10,
  nodeKind: "lesson",
  generatedBy: "test",
  hasEvidence: false,
};

const themes = [
  { motif: "buoy", scheme: "light" },
  { motif: "buoy", scheme: "dark" },
  { motif: "lighthouse", scheme: "light" },
  { motif: "lighthouse", scheme: "dark" },
] as const;

function chooseTheme(motif: "buoy" | "lighthouse", scheme: "light" | "dark") {
  act(() => {
    useThemeStore.getState().chooseMotif(motif);
    useThemeStore.getState().chooseIcons(motif);
    useThemeStore.getState().choose(scheme);
  });
}

for (const theme of themes) {
  it(`${theme.motif} ${theme.scheme} keeps the learning conversation readable`, () => {
    chooseTheme(theme.motif, theme.scheme);
    useLearningStore.setState({
      sessionId: "s",
      nodeId: node.id,
      lesson,
      messages: [{ id: "m1", role: "assistant", kind: "text", text: "我们从问题开始。" }],
      readyToComplete: false,
      mastery: 0.2,
      initialMastery: 0.2,
    });
    const { container, unmount } = render(
      <LearningScreen
        route={route}
        node={node}
        onBackToLesson={() => {}}
        onHome={() => {}}
        onFinish={async () => {}}
        reduced
      />,
    );
    expect(container.querySelector(".chat-page")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "发送消息" })).toBeVisible();
    unmount();
  });

  it(`${theme.motif} ${theme.scheme} keeps the lesson preview free of the full explanation`, () => {
    chooseTheme(theme.motif, theme.scheme);
    useLessonStore.setState({ byNode: { [node.id]: lesson }, loading: {}, error: null });
    const { unmount } = render(
      <LessonScreen
        route={route}
        node={node}
        index={0}
        tab="todo"
        onBack={() => {}}
        onTab={() => {}}
        onLaunch={() => {}}
        reduced
        launching={false}
      />,
    );
    expect(screen.getByRole("button", { name: /开始学习/ })).toBeVisible();
    expect(screen.queryByText(lesson.explanation)).toBeNull();
    unmount();
  });

  it(`${theme.motif} ${theme.scheme} keeps settings controls visible`, async () => {
    chooseTheme(theme.motif, theme.scheme);
    useSettingsStore.setState({ load: vi.fn(async () => {}) });
    useCloudStore.setState({ load: vi.fn(async () => {}), sessionState: "signedOut" });
    const { unmount } = render(<SettingsScreen onBack={() => {}} />);
    expect(await screen.findByText("账号与学涯云")).toBeVisible();
    unmount();
  });

  it(`${theme.motif} ${theme.scheme} keeps the create-route flow visible`, () => {
    chooseTheme(theme.motif, theme.scheme);
    const { unmount } = render(
      <CreateRouteScreen
        onBack={() => {}}
        onCreate={() => {}}
        submitting={false}
        searchConfigured={false}
      />,
    );
    expect(screen.getByText("你想学什么？")).toBeVisible();
    unmount();
  });
}
