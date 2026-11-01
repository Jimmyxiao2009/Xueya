import { StrictMode } from "react";
import { act, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { GeneratingScreen } from "@/screens/GeneratingScreen";
import { useJobStore } from "@/stores/jobStore";
afterEach(() => { vi.useRealTimers(); useJobStore.getState().reset(); });
it("shows actual zero progress and a recoverable failure", () => {
  useJobStore.setState({ progress: 0, error: "network error", phase: "building_outline" });
  render(<GeneratingScreen topic="测试主题" onCancel={() => {}} onReady={() => {}} />);
  expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "0");
  expect(screen.getByRole("alert")).toHaveTextContent("重试");
  expect(screen.getByRole("button", { name: "返回路线" })).toBeEnabled();
});
it("completion survives StrictMode and parent callback changes, fires once", () => {
  vi.useFakeTimers();
  useJobStore.setState({ job: { id:"job", kind:"course_create", status:"completed", progress:1, message:"ready", createdAt:"", resultJson:'{"courseId":"course"}' }, progress:1, phase:"ready", error:null });
  const first=vi.fn(), latest=vi.fn();
  const view=render(<StrictMode><GeneratingScreen topic="主题" onCancel={() => {}} onReady={first} /></StrictMode>);
  view.rerender(<StrictMode><GeneratingScreen topic="主题" onCancel={() => {}} onReady={latest} /></StrictMode>);
  act(() => vi.advanceTimersByTime(600));
  expect(first).not.toHaveBeenCalled(); expect(latest).toHaveBeenCalledExactlyOnceWith("course");
  act(() => vi.advanceTimersByTime(600)); expect(latest).toHaveBeenCalledTimes(1);
});
