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
import { useRouteStore } from "@/stores/routeStore";
import { useSourceStore } from "@/stores/sourceStore";
import { useLearningStore } from "@/stores/learningStore";
import { useLessonStore } from "@/stores/lessonStore";
import { useJobStore } from "@/stores/jobStore";
import { useNavStore } from "@/stores/uiStore";
import { useSettingsStore } from "@/stores/settingsStore";

function clearLearningWorkspace() {
  try {
    localStorage.removeItem("orbit.activeRouteId");
  } catch {
    /* Storage may be disabled. */
  }
  useLearningStore.getState().reset();
  useLessonStore.getState().invalidate();
  useJobStore.getState().reset();
  useRouteStore.setState({ routes: [], activeRouteId: null, details: {}, loading: false, error: null, adaptations: {}, noticeClosed: {}, scrollPositions: {} });
  useSourceStore.setState({ sources: [], recent: [], loading: false, error: null });
  useNavStore.setState({ view: "home", tab: "todo", activeNodeId: null, direction: 1 });
}

async function refreshSettingsForAccount(): Promise<void> {
  await useSettingsStore.getState().load();
}

async function reloadLocalWorkspace(): Promise<void> {
  await Promise.all([
    useRouteStore.getState().load(),
    useSourceStore.getState().load(),
  ]);
}


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

let accountGeneration = 0;

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
    const generation = accountGeneration;
    const safeSet = (patch: Partial<CloudState>) => {
      if (generation === accountGeneration) set(patch);
    };
    safeSet({ loading: true, sessionState: get().account?.state ?? "restoring" });
    try {
      const account = await api.cloudAccountState();
      if (generation !== accountGeneration) return;
      safeSet({ account, sessionState: account.state, loading: false, error: null });
        await refreshSettingsForAccount();
        if (generation !== accountGeneration) return;
      if (account.state === "authenticated") {
        await Promise.all([get().loadDevices(), get().loadUsage(), get().loadSyncStatus()]);
        if (generation !== accountGeneration) return;
        if (account.syncEnabled) await get().loadConflicts();
      } else {
        // DegradedOffline may still have a usage/entitlement snapshot. Keep it
        // visible, but never call authenticated endpoints while offline.
        safeSet({ devices: [], syncStatus: null, conflicts: [] });
      }
    } catch (error) {
      safeSet({
        loading: false,
        sessionState: "signedOut",
        error: cloudErrorText(errorOf(error)),
      });
    }
  },

  register: async (email, password) => {
    const generation = ++accountGeneration;
    const safeSet = (patch: Partial<CloudState>) => {
      if (generation === accountGeneration) set(patch);
    };
    safeSet({ busy: true, error: null });
    try {
      const account = await api.cloudRegister({ email, password });
      if (generation !== accountGeneration) return false;
      clearLearningWorkspace();
      safeSet({ account, sessionState: account.state, busy: false });
        await refreshSettingsForAccount();
        if (generation !== accountGeneration) return false;
        await reloadLocalWorkspace();
      if (account.state === "authenticated") {
        await Promise.all([get().loadDevices(), get().loadUsage()]);
      }
      return true;
    } catch (error) {
      safeSet({ busy: false, error: cloudErrorText(errorOf(error)) });
      return false;
    }
  },

  login: async (email, password) => {
    const generation = ++accountGeneration;
    const safeSet = (patch: Partial<CloudState>) => {
      if (generation === accountGeneration) set(patch);
    };
    safeSet({ busy: true, error: null });
    try {
      const account = await api.cloudLogin({ email, password });
      if (generation !== accountGeneration) return false;
      clearLearningWorkspace();
      safeSet({ account, sessionState: account.state, busy: false });
        await refreshSettingsForAccount();
        if (generation !== accountGeneration) return false;
        await reloadLocalWorkspace();
      if (account.state === "authenticated") {
        await Promise.all([get().loadDevices(), get().loadUsage()]);
      }
      return true;
    } catch (error) {
      safeSet({ busy: false, error: cloudErrorText(errorOf(error)) });
      return false;
    }
  },

  logout: async () => {
    const generation = ++accountGeneration;
    const safeSet = (patch: Partial<CloudState>) => {
      if (generation === accountGeneration) set(patch);
    };
    safeSet({ busy: true, error: null });
    try {
      const account = await api.cloudLogout();
      if (generation !== accountGeneration) return;
      clearLearningWorkspace();
      safeSet({
        account,
        sessionState: account.state,
        me: null,
        devices: [],
        usage: null,
        syncStatus: null,
        conflicts: [],
        busy: false,
        syncing: false,
        loading: false,
      });
        await refreshSettingsForAccount();
        await reloadLocalWorkspace();
    } catch (error) {
      safeSet({ busy: false, error: cloudErrorText(errorOf(error)) });
    }
  },

  loadDevices: async () => {
    const generation = accountGeneration;
    const safeSet = (patch: Partial<CloudState>) => {
      if (generation === accountGeneration) set(patch);
    };
    try {
      const devices = await api.cloudListDevices();
      safeSet({ devices, error: null });
    } catch (error) {
      safeSet({ error: cloudErrorText(errorOf(error)) });
    }
  },

  removeDevice: async (id) => {
    const generation = accountGeneration;
    const safeSet = (patch: Partial<CloudState>) => {
      if (generation === accountGeneration) set(patch);
    };
    try {
      await api.cloudRemoveDevice(id);
      if (generation !== accountGeneration) return;
      await get().loadDevices();
    } catch (error) {
      safeSet({ error: cloudErrorText(errorOf(error)) });
    }
  },

  loadUsage: async () => {
    const generation = accountGeneration;
    const safeSet = (patch: Partial<CloudState>) => {
      if (generation === accountGeneration) set(patch);
    };
    try {
      const usage = await api.cloudGetAiUsage();
      safeSet({ usage, error: null });
    } catch {
      // Usage is supplementary. Do not push an error toast on every load.
      safeSet({ usage: null });
    }
  },

  loadSyncStatus: async () => {
    const generation = accountGeneration;
    const safeSet = (patch: Partial<CloudState>) => {
      if (generation === accountGeneration) set(patch);
    };
    try {
      const syncStatus = await api.syncStatus();
      safeSet({ syncStatus });
    } catch {
      safeSet({ syncStatus: null });
    }
  },

  loadConflicts: async () => {
    const generation = accountGeneration;
    const safeSet = (patch: Partial<CloudState>) => {
      if (generation === accountGeneration) set(patch);
    };
    try {
      const conflicts = await api.syncConflicts();
      safeSet({ conflicts });
    } catch {
      safeSet({ conflicts: [] });
    }
  },

  syncNow: async () => {
    const generation = accountGeneration;
    const safeSet = (patch: Partial<CloudState>) => {
      if (generation === accountGeneration) set(patch);
    };
    safeSet({ syncing: true, error: null });
    try {
      if (get().account?.state !== "authenticated") {
        await get().load();
      }
      if (get().account?.state !== "authenticated") {
        safeSet({ syncing: false, error: "Cloud 暂不可用 · 本地学习不受影响" });
        return;
      }
      if (generation !== accountGeneration) return;
      const summary = await api.syncNow();
      if (generation !== accountGeneration) return;
      safeSet({ syncing: false });
      if (summary.skipped && summary.reason && summary.reason !== "sync_disabled") {
        safeSet({
          error: cloudErrorText({
            code: "CLOUD_UNAVAILABLE",
            message: summary.reason,
            retryable: true,
          }),
        });
      }
      if (generation !== accountGeneration) return;
      await get().loadSyncStatus();
      if (generation !== accountGeneration) return;
      await get().loadConflicts();
    } catch (error) {
      safeSet({ syncing: false, error: cloudErrorText(errorOf(error)) });
    }
  },

  syncFiles: async () => {
    const generation = accountGeneration;
    const safeSet = (patch: Partial<CloudState>) => {
      if (generation === accountGeneration) set(patch);
    };
    safeSet({ syncing: true, error: null });
    try {
      await api.syncFilesNow();
      safeSet({ syncing: false });
      if (generation !== accountGeneration) return;
      await get().loadSyncStatus();
    } catch (error) {
      safeSet({ syncing: false, error: cloudErrorText(errorOf(error)) });
    }
  },

  setSyncEnabled: async (enabled) => {
    const generation = accountGeneration;
    const safeSet = (patch: Partial<CloudState>) => {
      if (generation === accountGeneration) set(patch);
    };
    safeSet({ busy: true, error: null });
    try {
      const account = await api.cloudSetSyncEnabled(enabled);
      if (generation !== accountGeneration) return;
      safeSet({ account, sessionState: account.state, busy: false });
      if (generation !== accountGeneration) return;
      await get().loadSyncStatus();
      if (generation !== accountGeneration) return;
      if (enabled) await get().syncNow();
    } catch (error) {
      safeSet({ busy: false, error: cloudErrorText(errorOf(error)) });
    }
  },

  setBaseUrl: async (url) => {
    const generation = ++accountGeneration;
    const safeSet = (patch: Partial<CloudState>) => {
      if (generation === accountGeneration) set(patch);
    };
    safeSet({ busy: true, error: null });
    try {
      const account = await api.cloudSetBaseUrl(url);
      if (generation !== accountGeneration) return;
      safeSet({
        account,
        sessionState: account.state,
        me: null,
        devices: [],
        usage: null,
        syncStatus: null,
        conflicts: [],
        busy: false,
        syncing: false,
        loading: false,
      });
    } catch (error) {
      safeSet({ busy: false, error: cloudErrorText(errorOf(error)) });
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
