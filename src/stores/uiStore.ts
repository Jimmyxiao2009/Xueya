import { create } from "zustand";

export type View = "home" | "lesson" | "learning" | "create" | "generating";
export type LessonTab = "todo" | "materials";

interface NavState {
  view: View;
  tab: LessonTab;
  /** The node the lesson page is previewing. */
  activeNodeId: string | null;
  /** Which direction the next view change animates in. */
  direction: number;
  setView: (view: View, direction?: number) => void;
  setTab: (tab: LessonTab) => void;
  setActiveNode: (nodeId: string | null) => void;
}

export const useNavStore = create<NavState>((set) => ({
  view: "home",
  tab: "todo",
  activeNodeId: null,
  direction: 1,
  setView: (view, direction) =>
    set((state) => ({ view, direction: direction ?? state.direction })),
  setTab: (tab) => set({ tab }),
  setActiveNode: (activeNodeId) => set({ activeNodeId }),
}));

interface ToastState {
  message: string;
  token: number;
  show: (message: string) => void;
  clear: () => void;
}

export const useToastStore = create<ToastState>((set) => ({
  message: "",
  token: 0,
  show: (message) => set((state) => ({ message, token: state.token + 1 })),
  clear: () => set({ message: "" }),
}));

let toastTimer: number | undefined;

/** Imperative helper so non-React code (gestures) can raise a toast. */
export function toast(message: string): void {
  useToastStore.getState().show(message);
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => useToastStore.getState().clear(), 3000);
}
