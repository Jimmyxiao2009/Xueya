import { act } from "@testing-library/react";
import { expect, it } from "vitest";
import { useLearningStore } from "@/stores/learningStore";
import { backend } from "./fakeBackend";

it("reopening the same lesson resumes the same session without a second opening", async () => {
  const nodeId = backend.routes[0]?.currentNodeId;
  expect(nodeId).toBeTruthy();
  useLearningStore.getState().reset();

  await act(async () => {
    await useLearningStore.getState().start(nodeId!);
  });
  await act(async () => {
    await useLearningStore.getState().send("我理解了这个概念");
  });
  await act(async () => {
    await useLearningStore.getState().send("是的，我明白了");
  });

  const first = useLearningStore.getState();
  const sessionId = first.sessionId;
  const messageCount = first.messages.length;
  const firstMessage = first.messages[0]?.text;

  // Equivalent to leaving the lesson and recreating the runtime.
  useLearningStore.getState().reset();
  await act(async () => {
    await useLearningStore.getState().start(nodeId!);
  });

  const resumed = useLearningStore.getState();
  expect(resumed.sessionId).toBe(sessionId);
  expect(resumed.messages).toHaveLength(messageCount);
  expect(resumed.messages[0]?.text).toBe(firstMessage);
});
