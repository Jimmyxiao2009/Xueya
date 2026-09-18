import { create } from "zustand";
import {
  api,
  errorOf,
  type CloudAccountState,
  type CloudDevice,
  type CloudMe,
  type CloudSessionState,
  type CloudUsage,
  type SyncConflict,
  type SyncStatus,
} from "@/api";
import { cloudErrorText } from "@/lib/cloudErrors";

interface CloudState {
  account: CloudAccountState | null;
  /** Explicit session state so a transient restore never looks anonymous. */
  sessionState: CloudSessionState;
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
  sessionState: "restoring",
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
    set({ loading: true, sessionState: get().account?.state ?? "restoring" });
    try {
      const account = await api.cloudAccountState();
      set({ account, sessionState: account.state, loading: false, error: null });
      if (account.state === "authenticated") {
        await Promise.all([get().loadDevices(), get().loadUsage(), get().loadSyncStatus()]);
        if (account.syncEnabled) await get().loadConflicts();
      } else {
        // DegradedOffline may still have a usage/entitlement snapshot. Keep it
        // visible, but never call authenticated endpoints while offline.
        set({ devices: [], syncStatus: null, conflicts: [] });
      }
    } catch (error) {
      set({
        loading: false,
        sessionState: "signedOut",
        error: cloudErrorText(errorOf(error)),
      });
    }
  },

  register: async (email, password) => {
    set({ busy: true, error: null });
    try {
      const account = await api.cloudRegister({ email, password });
      set({ account, sessionState: account.state, busy: false });
      if (account.state === "authenticated") {
        await Promise.all([get().loadDevices(), get().loadUsage()]);
      }
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
      set({ account, sessionState: account.state, busy: false });
      if (account.state === "authenticated") {
        await Promise.all([get().loadDevices(), get().loadUsage()]);
      }
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
        sessionState: account.state,
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
      if (get().account?.state !== "authenticated") {
        await get().load();
      }
      if (get().account?.state !== "authenticated") {
        set({ syncing: false, error: "Cloud 暂不可用 · 本地学习不受影响" });
        return;
      }
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
      set({ account, sessionState: account.state, busy: false });
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
        sessionState: account.state,
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

  const recover = async (syncWhenAuthenticated: boolean) => {
    if (running) return;
    running = true;
    try {
      let account = useCloudStore.getState().account;
      // Startup, foreground, manual sync and periodic recovery all pass
      // through the same account_state path. A degraded session is retried
      // without ever asking the learner to sign in again.
      if (!account || account.state !== "authenticated") {
        await useCloudStore.getState().load();
        account = useCloudStore.getState().account;
      }
      if (
        syncWhenAuthenticated &&
        account?.state === "authenticated" &&
        account.syncEnabled
      ) {
        await api.syncNow();
      }
    } catch {
      // Cloud is optional: a foreground sync failure must not surface as an error.
    } finally {
      running = false;
    }
  };

  const onVisibilityChange = () => {
    if (document.visibilityState === "visible") void recover(true);
  };
  const onOnline = () => void recover(true);

  document.addEventListener("visibilitychange", onVisibilityChange);
  if (typeof window !== "undefined") {
    window.addEventListener("online", onOnline);
    const interval = window.setInterval(() => {
      const account = useCloudStore.getState().account;
      if (!account || account.state === "degradedOffline") {
        void recover(false);
      }
    }, 30_000);
    void recover(true);
    return () => {
      document.removeEventListener("visibilitychange", onVisibilityChange);
      window.removeEventListener("online", onOnline);
      window.clearInterval(interval);
    };
  }
  void recover(true);
  return () => document.removeEventListener("visibilitychange", onVisibilityChange);
}
