import { create } from "zustand";
import {
  api,
  errorOf,
  type AdaptationEvent,
  type CourseNodeComplete,
  type LearningRoute,
  type RouteCreateInput,
  type RouteDetail,
} from "@/api";

const ACTIVE_ROUTE_KEY = "orbit.activeRouteId";

function savedActiveRoute(): string | null {
  try {
    return localStorage.getItem(ACTIVE_ROUTE_KEY);
  } catch {
    return null;
  }
}

export function rememberActiveRoute(id: string | null) {
  try {
    if (id) localStorage.setItem(ACTIVE_ROUTE_KEY, id);
    else localStorage.removeItem(ACTIVE_ROUTE_KEY);
  } catch {
    /* Storage may be disabled; the in-memory selection still works. */
  }
}

interface RouteState {
  routes: LearningRoute[];
  activeRouteId: string | null;
  details: Record<string, RouteDetail>;
  loading: boolean;
  error: string | null;
  /** Scroll offsets are part of the swipe design: returning home restores them. */
  scrollPositions: Record<string, number>;
  /** Newest-first roadmap changes, per route, used by the notice strip. */
  adaptations: Record<string, AdaptationEvent[]>;
  /** Notices the learner has already dismissed, per route. */
  noticeClosed: Record<string, boolean>;
  load: () => Promise<void>;
  setActive: (id: string) => void;
  detail: (id: string) => RouteDetail | undefined;
  loadDetail: (id: string) => Promise<RouteDetail | undefined>;
  createRoute: (input: RouteCreateInput) => Promise<string | null>;
  removeRoute: (id: string) => Promise<void>;
  completeNode: (nodeId: string) => Promise<CourseNodeComplete | null>;
  rememberScroll: (id: string, top: number) => void;
  loadAdaptations: (id: string) => Promise<void>;
  dismissNotice: (id: string) => void;
  activeRoute: () => LearningRoute | undefined;
}

export const useRouteStore = create<RouteState>((set, get) => ({
  routes: [],
  activeRouteId: savedActiveRoute(),
  details: {},
  loading: false,
  error: null,
  scrollPositions: {},
  adaptations: {},
  noticeClosed: {},

  load: async () => {
    set({ loading: true, error: null });
    try {
      const routes = await api.routeList();
      const current = get().activeRouteId;
      const activeRouteId =
        current && routes.some((route) => route.id === current)
          ? current
          : (routes[0]?.id ?? null);
      rememberActiveRoute(activeRouteId);
      set({ routes, activeRouteId, loading: false });
      // Every route's roadmap is loaded up front: a swipe preview has to render
      // the neighbouring route synchronously, mid-gesture.
      await Promise.all(
        routes.map((route) =>
          get().details[route.id] ? Promise.resolve() : get().loadDetail(route.id),
        ),
      );
      await Promise.all(routes.map((route) => get().loadAdaptations(route.id)));
    } catch (error) {
      set({ loading: false, error: errorOf(error).message });
    }
  },

  setActive: (id) => {
    rememberActiveRoute(id);
    set({ activeRouteId: id });
  },

  detail: (id) => get().details[id],

  loadDetail: async (id) => {
    try {
      const detail = await api.routeGet(id);
      set((state) => ({
        details: { ...state.details, [id]: detail },
        routes: state.routes.map((route) =>
          route.id === id ? detail.course : route,
        ),
      }));
      return detail;
    } catch (error) {
      set({ error: errorOf(error).message });
      return undefined;
    }
  },

  createRoute: async (input) => {
    try {
      const job = await api.routeCreate(input);
      return job.id;
    } catch (error) {
      set({ error: errorOf(error).message });
      return null;
    }
  },

  removeRoute: async (id) => {
    await api.routeDelete(id);
    set((state) => {
      const details = { ...state.details };
      delete details[id];
      const routes = state.routes.filter((route) => route.id !== id);
      const activeRouteId =
        state.activeRouteId === id ? (routes[0]?.id ?? null) : state.activeRouteId;
      return { routes, details, activeRouteId };
    });
    rememberActiveRoute(get().activeRouteId);
    const active = get().activeRouteId;
    if (active && !get().details[active]) {
      await get().loadDetail(active);
    }
  },

  completeNode: async (nodeId) => {
    try {
      const result = await api.nodeComplete(nodeId);
      const detail = await api.routeGet(result.course.id);
      set((state) => ({
        details: { ...state.details, [result.course.id]: detail },
        routes: state.routes.map((route) =>
          route.id === result.course.id ? result.course : route,
        ),
      }));
      await get().loadAdaptations(result.course.id);
      return result;
    } catch (error) {
      set({ error: errorOf(error).message });
      return null;
    }
  },

  rememberScroll: (id, top) =>
    set((state) => ({ scrollPositions: { ...state.scrollPositions, [id]: top } })),

  loadAdaptations: async (id) => {
    try {
      const events = await api.routeAdaptations(id, 8);
      set((state) => ({ adaptations: { ...state.adaptations, [id]: events } }));
    } catch {
      // A route without adaptation history is not an error.
    }
  },

  dismissNotice: (id) =>
    set((state) => ({ noticeClosed: { ...state.noticeClosed, [id]: true } })),

  activeRoute: () => {
    const { routes, activeRouteId } = get();
    return routes.find((route) => route.id === activeRouteId);
  },
}));
