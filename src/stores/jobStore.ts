import { create } from "zustand";
import { api, errorOf, type Job } from "@/api";

export type JobPhase =
  | "collecting_sources"
  | "parsing"
  | "building_outline"
  | "building_roadmap"
  | "ready";

export interface JobState {
  job: Job | null;
  phase: JobPhase;
  message: string;
  progress: number;
  error: string | null;
  start: (jobId: string) => Promise<Job | null>;
  apply: (job: Job) => void;
  fail: (message: string) => void;
  reset: () => void;
}

const PHASE_COPY: Record<JobPhase, string> = {
  collecting_sources: "正在整理你选择的资料…",
  parsing: "正在解析资料内容…",
  building_outline: "正在拆分章节与安排复习…",
  building_roadmap: "正在铺好学习路线…",
  ready: "路线已准备好",
};

/** Map real backend progress onto the four phases the prototype pretended to have. */
export function phaseOf(job: Job): JobPhase {
  if (job.status === "completed") return "ready";
  const text = `${job.kind} ${job.message}`.toLowerCase();
  if (text.includes("整理学习") || text.includes("evidence") || text.includes("collecting")) return "collecting_sources";
  if (text.includes("规划大纲") || text.includes("plan") || text.includes("outline") || text.includes("requesting")) {
    return "building_outline";
  }
  if (text.includes("saving") || text.includes("roadmap")) return "building_roadmap";
  if (job.progress >= 0.6) return "building_roadmap";
  return "parsing";
}

export function jobPhaseCopy(phase: JobPhase): string {
  return PHASE_COPY[phase];
}

export const useJobStore = create<JobState>((set) => ({
  job: null,
  phase: "collecting_sources",
  message: PHASE_COPY.collecting_sources,
  progress: 0.05,
  error: null,

  start: async (jobId) => {
    set({
      job: null,
      phase: "collecting_sources",
      message: PHASE_COPY.collecting_sources,
      progress: 0.05,
      error: null,
    });
    try {
      const job = await api.jobGet(jobId);
      const phase = phaseOf(job);
      set({ job, phase, message: PHASE_COPY[phase], progress: job.progress });
      return job;
    } catch (error) {
      set({ error: errorOf(error).message });
      return null;
    }
  },

  apply: (job) => {
    const phase = phaseOf(job);
    set({
      job,
      phase,
      message: job.status === "failed" ? "路线生成失败" : PHASE_COPY[phase],
      progress: job.progress,
      error: job.status === "failed" ? (job.errorJson ?? "路线生成失败") : null,
    });
  },

  fail: (message) => set({ error: message }),

  reset: () =>
    set({
      job: null,
      phase: "collecting_sources",
      message: PHASE_COPY.collecting_sources,
      progress: 0.05,
      error: null,
    }),
}));
