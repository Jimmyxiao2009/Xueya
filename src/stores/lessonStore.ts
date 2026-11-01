import { create } from "zustand";
import { api, errorOf, type Lesson } from "@/api";

interface LessonState {
  byNode: Record<string, Lesson>;
  loading: Record<string, boolean>;
  error: string | null;
  /** Loads (and caches) the prepared lesson for a node. */
  prepare: (nodeId: string, force?: boolean) => Promise<Lesson | null>;
  invalidate: (nodeId?: string) => void;
}

export const useLessonStore = create<LessonState>((set, get) => ({
  byNode: {},
  loading: {},
  error: null,

  prepare: async (nodeId, force = false) => {
    const cached = get().byNode[nodeId];
    if (cached && !force) return cached;
    if (get().loading[nodeId]) return get().byNode[nodeId] ?? null;
    set((state) => ({ loading: { ...state.loading, [nodeId]: true }, error: null }));
    try {
      const lesson = await api.nodePrepare(nodeId);
      set((state) => ({
        byNode: { ...state.byNode, [nodeId]: lesson },
        loading: { ...state.loading, [nodeId]: false },
        error: null,
      }));
      return lesson;
    } catch (error) {
      set((state) => ({
        loading: { ...state.loading, [nodeId]: false },
        error: errorOf(error).message,
      }));
      return null;
    }
  },

  invalidate: (nodeId) =>
    set((state) => {
      if (!nodeId) return { byNode: {} };
      const byNode = { ...state.byNode };
      delete byNode[nodeId];
      return { byNode };
    }),
}));
