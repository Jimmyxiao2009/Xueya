import { create } from "zustand";
import { api, errorOf, watchJob, type Source } from "@/api";

interface SourceState {
  sources: Source[];
  loading: boolean;
  error: string | null;
  /** Ids imported in this session, newest first, so the form can show them. */
  recent: string[];
  load: () => Promise<void>;
  importFile: (file: File) => Promise<string | null>;
  importUrl: (url: string, title?: string) => Promise<string | null>;
  remove: (id: string) => Promise<void>;
}

function base64Of(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  const chunk = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunk) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunk));
  }
  return btoa(binary);
}

/**
 * Sources are the only real "input" the app has. Imports run as jobs, and the
 * job's `resultJson` carries the new source id so a route can be grounded in it
 * without guessing from the file list.
 */
export const useSourceStore = create<SourceState>((set, get) => ({
  sources: [],
  loading: false,
  error: null,
  recent: [],

  load: async () => {
    set({ loading: true });
    try {
      const sources = await api.sourceList();
      set({ sources, loading: false, error: null });
    } catch (error) {
      set({ loading: false, error: errorOf(error).message });
    }
  },

  importFile: async (file) => {
    try {
      const bytesBase64 = base64Of(await file.arrayBuffer());
      const job = await api.sourceImportBytes({
        filename: file.name,
        bytesBase64,
        mimeType: file.type || undefined,
      });
      const finished = await watchJob(job.id, () => undefined);
      if (finished.status !== "completed") {
        set({ error: "资料导入失败" });
        return null;
      }
      const sourceId = readSourceId(finished.resultJson);
      await get().load();
      if (sourceId) set((state) => ({ recent: [sourceId, ...state.recent] }));
      return sourceId;
    } catch (error) {
      set({ error: errorOf(error).message });
      return null;
    }
  },

  importUrl: async (url, title) => {
    try {
      const job = await api.sourceImportUrl({ url, title });
      const finished = await watchJob(job.id, () => undefined);
      if (finished.status !== "completed") {
        set({ error: "链接读取失败" });
        return null;
      }
      const sourceId = readSourceId(finished.resultJson);
      await get().load();
      if (sourceId) set((state) => ({ recent: [sourceId, ...state.recent] }));
      return sourceId;
    } catch (error) {
      set({ error: errorOf(error).message });
      return null;
    }
  },

  remove: async (id) => {
    try {
      await api.sourceDelete(id);
      set((state) => ({
        sources: state.sources.filter((source) => source.id !== id),
        recent: state.recent.filter((value) => value !== id),
      }));
    } catch (error) {
      set({ error: errorOf(error).message });
    }
  },
}));

function readSourceId(resultJson: string | null | undefined): string | null {
  if (!resultJson) return null;
  try {
    const parsed = JSON.parse(resultJson) as { sourceId?: string };
    return parsed.sourceId ?? null;
  } catch {
    return null;
  }
}
