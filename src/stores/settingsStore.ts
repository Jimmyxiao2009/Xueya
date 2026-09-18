import { create } from "zustand";
import { api, errorOf, type Provider, type ProviderCapabilities } from "@/api";

export interface ProviderDraft {
  id?: string;
  displayName: string;
  baseUrl: string;
  chatModel: string;
  embeddingModel: string;
  apiKey: string;
  enabled: boolean;
}

export interface ProviderProbe {
  ok: boolean;
  message: string;
  protocol?: string | null;
  capabilities?: ProviderCapabilities | null;
}

interface SettingsState {
  providers: Provider[];
  searchProvider: string;
  hasBraveApiKey: boolean;
  aiMode: string;
  loading: boolean;
  error: string | null;
  /** Result of the last `provider_test`, keyed by provider id. */
  probes: Record<string, ProviderProbe>;
  load: () => Promise<void>;
  save: (draft: ProviderDraft) => Promise<boolean>;
  remove: (id: string) => Promise<void>;
  test: (id: string) => Promise<void>;
  saveSearch: (input: { searchProvider?: string; braveApiKey?: string }) => Promise<void>;
  setAiMode: (mode: string) => Promise<void>;
}

export const EMPTY_DRAFT: ProviderDraft = {
  displayName: "",
  baseUrl: "https://api.openai.com/v1",
  chatModel: "gpt-4o-mini",
  embeddingModel: "text-embedding-3-small",
  apiKey: "",
  enabled: true,
};

/**
 * BYOK settings.
 *
 * The API key travels one way only: it is sent on save and never comes back.
 * `provider_list` returns `hasApiKey`, which is all the UI needs to show
 * "已保存 · 更换" instead of the key itself.
 */
export const useSettingsStore = create<SettingsState>((set, get) => ({
  providers: [],
  searchProvider: "none",
  hasBraveApiKey: false,
  aiMode: "offline",
  loading: false,
  error: null,
  probes: {},

  load: async () => {
    set({ loading: true });
    try {
      const [providers, settings] = await Promise.all([
        api.providerList(),
        api.settingsGet(),
      ]);
      set({
        providers,
        searchProvider: settings.searchProvider,
        hasBraveApiKey: settings.hasBraveApiKey,
        aiMode: settings.aiMode,
        loading: false,
        error: null,
      });
    } catch (error) {
      set({ loading: false, error: errorOf(error).message });
    }
  },

  save: async (draft) => {
    try {
      await api.providerUpsert({
        id: draft.id,
        displayName: draft.displayName.trim(),
        baseUrl: draft.baseUrl.trim(),
        chatModel: draft.chatModel.trim(),
        embeddingModel: draft.embeddingModel.trim(),
        apiKey: draft.apiKey.trim() || undefined,
        enabled: draft.enabled,
      });
      set({ error: null });
      await get().load();
      return true;
    } catch (error) {
      set({ error: errorOf(error).message });
      return false;
    }
  },

  remove: async (id) => {
    try {
      await api.providerDelete(id);
      await get().load();
    } catch (error) {
      set({ error: errorOf(error).message });
    }
  },

  test: async (id) => {
    try {
      const result = await api.providerTest(id);
      set((state) => ({ probes: { ...state.probes, [id]: result } }));
    } catch (error) {
      set((state) => ({
        probes: {
          ...state.probes,
          [id]: { ok: false, message: errorOf(error).message },
        },
      }));
    }
  },

  saveSearch: async (input) => {
    try {
      const result = await api.settingsUpdate(input);
      set({
        searchProvider: result.searchProvider,
        hasBraveApiKey: result.hasBraveApiKey,
        aiMode: result.aiMode,
        error: null,
      });
    } catch (error) {
      set({ error: errorOf(error).message });
    }
  },

  setAiMode: async (mode) => {
    try {
      const result = await api.settingsUpdate({ aiMode: mode });
      set({ aiMode: result.aiMode, error: null });
    } catch (error) {
      set({ error: errorOf(error).message });
    }
  },
}));
