import { create } from "zustand";
import { api, type AppInfo } from "@/api";

interface AppState {
  info: AppInfo | null;
  refresh: () => Promise<void>;
}

/**
 * Bootstrap facts the shell needs before it can offer an action: how many
 * sources exist, and whether web search is usable. `searchConfigured` is what
 * lets the create form promise "联网搜集资料" only when it can deliver it.
 */
export const useAppStore = create<AppState>((set) => ({
  info: null,
  refresh: async () => {
    try {
      set({ info: await api.bootstrap() });
    } catch {
      // A missing bootstrap only means the optional affordances stay hidden.
    }
  },
}));
