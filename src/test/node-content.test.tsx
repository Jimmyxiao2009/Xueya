import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { api } from "@/api";
import { LessonScreen } from "@/components/lesson/LessonScreen";
import { useLessonStore } from "@/stores/lessonStore";

it("offers a retry after content fails without starting a lesson or retrying on metadata refresh", async () => {
  const route = (await api.routeList())[0];
  const node = (await api.routeGet(route.id)).nodes[0];
  useLessonStore.setState({ byNode: {}, loading: {}, error: null });
  const prepare = vi.spyOn(api, "nodePrepare").mockRejectedValue(new Error("upstream unavailable"));
  const props = { route, node, index: 0, tab: "todo" as const, onBack: vi.fn(), onTab: vi.fn(), onLaunch: vi.fn(), reduced: true, launching: false };
  const view = render(<LessonScreen {...props} />);
  await screen.findByRole("button", { name: "重试本节内容" });
  expect(screen.getByRole("button", { name: /开始学习/ })).toBeDisabled();
  view.rerender(<LessonScreen {...props} node={{ ...node, metadataJson: '{"contentStatus":"failed"}' }} />);
  expect(prepare).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("button", { name: "重试本节内容" }));
  await waitFor(() => expect(prepare).toHaveBeenCalledTimes(2));
  expect(props.onLaunch).not.toHaveBeenCalled();
  prepare.mockRestore();
});
