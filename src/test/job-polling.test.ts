import { afterEach, expect, it, vi } from "vitest";
import { api, watchJob, type Job } from "@/api";

afterEach(() => { vi.restoreAllMocks(); vi.useRealTimers(); });

it("recovers from a transient progress fetch failure and returns the completed outline", async () => {
  vi.useFakeTimers();
  const finished = { id:"outline", status:"completed", resultJson:'{"courseId":"route"}' } as Job;
  const get = vi.spyOn(api, "jobGet").mockRejectedValueOnce(new Error("network"))
    .mockResolvedValueOnce({ ...finished, status:"running" }).mockResolvedValueOnce(finished);
  const update = vi.fn();
  const result = watchJob("outline", update);
  await vi.runAllTimersAsync();
  expect(await result).toEqual(finished);
  expect(get).toHaveBeenCalledTimes(3);
  expect(update).toHaveBeenLastCalledWith(finished);
});

it("stops retrying persistent progress errors so the caller can show recovery UI", async () => {
  vi.useFakeTimers();
  const get = vi.spyOn(api, "jobGet").mockRejectedValue(new Error("offline"));
  const result = expect(watchJob("outline", vi.fn())).rejects.toThrow("offline");
  await vi.runAllTimersAsync();
  await result;
  expect(get).toHaveBeenCalledTimes(5);
});
