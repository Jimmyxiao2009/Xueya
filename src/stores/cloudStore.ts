import { create } from "zustand";
import {
  api,
  errorOf,
  type CloudAccountState,
  type CloudDevice,
  type CloudMe,
  type CloudUsage,
  type SyncConflict,
  type SyncStatus,
} from "@/api";
import { cloudErrorText } from "@/lib/cloudErrors";

interface CloudState {
  account: CloudAccountState | null;
  me: CloudMe | null;
  devices: CloudDevice[];
  usage: CloudUsage | null;
  syncStatus: SyncStatus | null;
  conflicts: SyncConflict[];
  syncing: boolean;
  loading: boolean;
  busy: boolean;
  error: string | null;
  load: () => Promise<void>;
  register: (email: string, password: string) => Promise<boolean>;
  login: (email: string, password: string) => Promise<boolean>;
  logout: () => Promise<void>;
  loadDevices: () => Promise<void>;
  removeDevice: (id: string) => Promise<void>;
  loadUsage: () => Promise<void>;
  loadSyncStatus: () => Promise<void>;
  loadConflicts: () => Promise<void>;
  syncNow: () => Promise<void>;
  syncFiles: () => Promise<void>;
  setSyncEnabled: (enabled: boolean) => Promise<void>;
  setBaseUrl: (url: string) => Promise<void>;
}

export const useCloudStore = create<CloudState>((set, get) => ({
  account: null,
  me: null,
  devices: [],
  usage: null,
  syncStatus: null,
  conflicts: [],
  syncing: false,
  loading: false,
  busy: false,
  error: null,

  load: async () => {
    set({ loading: true });
    try {
      const account = await api.cloudAccountState();
      set({ account, loading: false, error: null });
      if (account.loggedIn) {
        await Promise.all([get().loadDevices(), get().loadUsage(), get().loadSyncStatus()]);
        if (account.syncEnabled) await get().loadConflicts();
      }
    } catch (error) {
      set({ loading: false, error: cloudErrorText(errorOf(error)) });
    }
  },

  register: async (email, password) => {
    set({ busy: true, error: null });
    try {
      const account = await api.cloudRegister({ email, password });
      set({ account, busy: false });
      await Promise.all([get().loadDevices(), get().loadUsage()]);
      return true;
    } catch (error) {
      set({ busy: false, error: cloudErrorText(errorOf(error)) });
      return false;
    }
  },

  login: async (email, password) => {
    set({ busy: true, error: null });
    try {
      const account = await api.cloudLogin({ email, password });
      set({ account, busy: false });
      await Promise.all([get().loadDevices(), get().loadUsage()]);
      return true;
    } catch (error) {
      set({ busy: false, error: cloudErrorText(errorOf(error)) });
      return false;
    }
  },

  logout: async () => {
    set({ busy: true, error: null });
    try {
      const account = await api.cloudLogout();
      set({
        account,
        me: null,
        devices: [],
        usage: null,
        syncStatus: null,
        conflicts: [],
        busy: false,
      });
    } catch (error) {
      set({ busy: false, error: cloudErrorText(errorOf(error)) });
    }
  },

  loadDevices: async () => {
    try {
      const devices = await api.cloudListDevices();
      set({ devices, error: null });
    } catch (error) {
      set({ error: cloudErrorText(errorOf(error)) });
    }
  },

  removeDevice: async (id) => {
    try {
      await api.cloudRemoveDevice(id);
      await get().loadDevices();
    } catch (error) {
      set({ error: cloudErrorText(errorOf(error)) });
    }
  },

  loadUsage: async () => {
    try {
      const usage = await api.cloudGetAiUsage();
      set({ usage, error: null });
    } catch {
      // Usage is supplementary. Do not push an error toast on every load.
      set({ usage: null });
    }
  },

  loadSyncStatus: async () => {
    try {
      const syncStatus = await api.syncStatus();
      set({ syncStatus });
    } catch {
      set({ syncStatus: null });
    }
  },

  loadConflicts: async () => {
    try {
      const conflicts = await api.syncConflicts();
      set({ conflicts });
    } catch {
      set({ conflicts: [] });
    }
  },

  syncNow: async () => {
    set({ syncing: true, error: null });
    try {
      const summary = await api.syncNow();
      set({ syncing: false });
      if (summary.skipped && summary.reason && summary.reason !== "sync_disabled") {
        set({
          error: cloudErrorText({
            code: "CLOUD_UNAVAILABLE",
            message: summary.reason,
            retryable: true,
          }),
        });
      }
      await get().loadSyncStatus();
      await get().loadConflicts();
    } catch (error) {
      set({ syncing: false, error: cloudErrorText(errorOf(error)) });
    }
  },

  syncFiles: async () => {
    set({ syncing: true, error: null });
    try {
      await api.syncFilesNow();
      set({ syncing: false });
      await get().loadSyncStatus();
    } catch (error) {
      set({ syncing: false, error: cloudErrorText(errorOf(error)) });
    }
  },

  setSyncEnabled: async (enabled) => {
    set({ busy: true, error: null });
    try {
      const account = await api.cloudSetSyncEnabled(enabled);
      set({ account, busy: false });
      await get().loadSyncStatus();
      if (enabled) await get().syncNow();
    } catch (error) {
      set({ busy: false, error: cloudErrorText(errorOf(error)) });
    }
  },

  setBaseUrl: async (url) => {
    set({ busy: true, error: null });
    try {
      const account = await api.cloudSetBaseUrl(url);
      set({
        account,
        me: null,
        devices: [],
        usage: null,
        syncStatus: null,
        conflicts: [],
        busy: false,
      });
    } catch (error) {
      set({ busy: false, error: cloudErrorText(errorOf(error)) });
    }
  },
}));

/**
 * Local-first foreground sync: when the WebView becomes visible again, ask the
 * backend for a debounced push/pull. A Cloud failure is deliberately swallowed
 * because learning must keep working offline. Returns an unsubscribe function.
 */
export function startForegroundSync(): () => void {
  if (typeof document === "undefined") return () => {};
  let running = false;
  const onVisibilityChange = async () => {
    if (document.visibilityState !== "visible" || running) return;
    running = true;
    try {
      const state = useCloudStore.getState();
      if (!state.account) await state.load();
      const account = useCloudStore.getState().account;
      if (account?.loggedIn && account.syncEnabled) await api.syncNow();
    } catch {
      // Cloud is optional: a foreground sync failure must not surface as an error.
    } finally {
      running = false;
    }
  };
  document.addEventListener("visibilitychange", onVisibilityChange);
  return () => document.removeEventListener("visibilitychange", onVisibilityChange);
}
