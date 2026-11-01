import { act } from "@testing-library/react";
import { expect, it } from "vitest";
import { completedSteps, useLearningStore } from "@/stores/learningStore";
import { backend } from "./fakeBackend";

it("conversation can reach ready-to-complete without opening the optional quiz", async () => {
  const nodeId = backend.routes[0]?.currentNodeId;
  expect(nodeId).toBeTruthy();
  useLearningStore.getState().reset();

  await act(async () => {
    expect(await useLearningStore.getState().start(nodeId!)).toBe(true);
  });

  for (const reply of ["我理解了这个概念", "是的，我明白了", "我可以总结一下"]) {
    await act(async () => {
      await useLearningStore.getState().send(reply);
    });
  }

  const state = useLearningStore.getState();
  expect(state.readyToComplete).toBe(true);
  expect(state.mastery).toBeGreaterThanOrEqual(0.75);
  expect(completedSteps(state)).toBe(state.lesson?.steps.length ?? 0);
});
