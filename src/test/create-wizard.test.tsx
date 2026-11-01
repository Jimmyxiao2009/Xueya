import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { CreateRouteScreen } from "@/screens/CreateRouteScreen";
import { useSourceStore } from "@/stores/sourceStore";
import { api } from "@/api";

afterEach(() => vi.restoreAllMocks());
function start(searchConfigured = false) {
  const create = vi.fn(async () => {});
  render(<CreateRouteScreen onBack={() => {}} onCreate={create} submitting={false} searchConfigured={searchConfigured} />);
  expect(screen.getAllByRole("textbox")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: /线性代数/ }));
  expect(screen.getByLabelText("想学的主题")).toHaveValue("线性代数");
  fireEvent.click(screen.getByRole("button", { name: "继续" }));
  return create;
}
it("keeps both slider selections through back navigation and submits on the third page", async () => {
  const create = start();
  const sliders = screen.getAllByRole("slider");
  expect(sliders).toHaveLength(2);
  fireEvent.change(sliders[0], { target: { value: "1" } });
  fireEvent.change(sliders[1], { target: { value: "2" } });
  fireEvent.click(screen.getByRole("button", { name: "返回上一步" }));
  fireEvent.click(screen.getByRole("button", { name: "继续" }));
  expect(screen.getAllByRole("slider")[0]).toHaveValue("1");
  expect(screen.getAllByRole("slider")[1]).toHaveValue("2");
  fireEvent.click(screen.getByRole("button", { name: "继续" }));
  expect(screen.getByRole("button", { name: "帮我找资料" })).toBeDisabled();
  expect(create).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "生成我的路线" }));
  await waitFor(() => expect(create).toHaveBeenCalledWith(expect.objectContaining({ topic: "线性代数", experienceLevel: "some_experience", desiredDepth: "deep", sourceIds: [], collectWeb: false })));
});
it("skip uses the default starting point and depth", async () => {
  const create = start();
  fireEvent.click(screen.getByRole("button", { name: "已有基础" }));
  fireEvent.click(screen.getByRole("button", { name: "深入掌握" }));
  fireEvent.click(screen.getByRole("button", { name: "跳过" }));
  fireEvent.click(screen.getByRole("button", { name: "生成我的路线" }));
  await waitFor(() => expect(create).toHaveBeenCalledWith(expect.objectContaining({ experienceLevel: "beginner", desiredDepth: "practical" })));
});
it("AI discovery imports real search hits and submits only successfully read sources", async () => {
  const search = vi.spyOn(api, "searchWeb").mockResolvedValue([
    { title: "University course", url: "https://example.org/course", snippet: "Lecture notes", source: "step" },
    { title: "Unavailable page", url: "https://example.org/fail", snippet: "Text", source: "step" },
  ]);
  const importUrl = vi.fn().mockResolvedValueOnce("source-1").mockResolvedValueOnce(null);
  useSourceStore.setState({ importUrl });
  const create = start(true);
  fireEvent.click(screen.getByRole("button", { name: "跳过" }));
  fireEvent.click(screen.getByRole("button", { name: "帮我找资料" }));
  await screen.findByText(/读取失败，可移除后重试/);
  expect(search).toHaveBeenCalledWith(expect.objectContaining({ ai: true, query: expect.stringContaining("线性代数") }));
  expect(importUrl).toHaveBeenCalledTimes(2);
  fireEvent.click(screen.getByRole("button", { name: "生成我的路线" }));
  await waitFor(() => expect(create).toHaveBeenCalledWith(expect.objectContaining({ sourceIds: ["source-1"] })));
});

