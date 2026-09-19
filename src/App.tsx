import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactElement,
} from "react";
import { createRoot, type Root } from "react-dom/client";
import { flushSync } from "react-dom";
import { watchJob } from "@/api";
import { AppHeader } from "@/components/layout/AppHeader";
import { Toast } from "@/components/layout/Toast";
import { Lighthouse } from "@/components/ui/Lighthouse";
import { RoadmapLayer } from "@/components/roadmap/RoadmapLayer";
import { RoadmapScreen } from "@/components/roadmap/RoadmapScreen";
import { NoticeBanner } from "@/components/roadmap/NoticeBanner";
import { LessonScreen } from "@/components/lesson/LessonScreen";
import { LessonPane } from "@/components/lesson/LessonPane";
import { LearningScreen } from "@/components/learning/LearningScreen";
import {
  CreateRouteScreen,
  type RouteCreateFormInput,
} from "@/screens/CreateRouteScreen";
import { GeneratingScreen } from "@/screens/GeneratingScreen";
import { SettingsScreen } from "@/screens/SettingsScreen";
import { SourcesScreen } from "@/screens/SourcesScreen";
import { useControlFeedback } from "@/hooks/useControlFeedback";
import { useReducedMotion, prefersReducedMotion } from "@/hooks/useReducedMotion";
import { useVisualViewport } from "@/hooks/useVisualViewport";
import { animateLessonTransition, lessonCardClip } from "@/lib/lessonTransition";
import {
  bindSwipeZone,
  type SwipePreview,
  type SwipeSpec,
} from "@/lib/swipeEngine";
import { useLearningStore } from "@/stores/learningStore";
import { useLessonStore } from "@/stores/lessonStore";
import { useJobStore } from "@/stores/jobStore";
import { useRouteStore } from "@/stores/routeStore";
import { useAppStore } from "@/stores/appStore";
import { useSourceStore } from "@/stores/sourceStore";
import { toast, useNavStore } from "@/stores/uiStore";

type PendingTransition =
  | { kind: "open"; underlay: HTMLElement; clip: string }
  | { kind: "close"; opening: HTMLElement };

/** Renders a component into a detached stage and returns its root element. */
function mountPreview(
  host: HTMLElement,
  render: () => ReactElement,
): { root: Root; surface: HTMLElement | null } {
  const root = createRoot(host);
  flushSync(() => root.render(render()));
  return { root, surface: (host.firstElementChild as HTMLElement | null) ?? null };
}

export default function App() {
  useVisualViewport();
  useControlFeedback();
  const reduced = useReducedMotion();

  const view = useNavStore((state) => state.view);
  const tab = useNavStore((state) => state.tab);
  const activeNodeId = useNavStore((state) => state.activeNodeId);
  const direction = useNavStore((state) => state.direction);
  const setView = useNavStore((state) => state.setView);
  const setTab = useNavStore((state) => state.setTab);
  const setActiveNode = useNavStore((state) => state.setActiveNode);

  const routes = useRouteStore((state) => state.routes);
  const activeRouteId = useRouteStore((state) => state.activeRouteId);
  const details = useRouteStore((state) => state.details);
  const scrollPositions = useRouteStore((state) => state.scrollPositions);
  const adaptations = useRouteStore((state) => state.adaptations);
  const noticeClosed = useRouteStore((state) => state.noticeClosed);
  const loadRoutes = useRouteStore((state) => state.load);
  const setActiveRoute = useRouteStore((state) => state.setActive);
  const rememberScroll = useRouteStore((state) => state.rememberScroll);
  const removeRoute = useRouteStore((state) => state.removeRoute);
  const completeNode = useRouteStore((state) => state.completeNode);
  const dismissNotice = useRouteStore((state) => state.dismissNotice);
  const loadDetail = useRouteStore((state) => state.loadDetail);
  const loadAdaptations = useRouteStore((state) => state.loadAdaptations);

  const sources = useSourceStore((state) => state.sources);
  const loadSources = useSourceStore((state) => state.load);
  const collectSources = useSourceStore((state) => state.collect);
  const searchConfigured = useAppStore((state) => state.info?.searchConfigured ?? false);
  const refreshInfo = useAppStore((state) => state.refresh);

  const lessons = useLessonStore((state) => state.byNode);
  const prepareLesson = useLessonStore((state) => state.prepare);
  const learning = useLearningStore();

  const startJob = useJobStore((state) => state.start);
  const applyJob = useJobStore((state) => state.apply);
  const resetJob = useJobStore((state) => state.reset);

  const screenRef = useRef<HTMLDivElement>(null);
  const homeScrollRef = useRef<HTMLElement>(null);
  const lessonScrollRef = useRef(0);
  const scrollRef = useRef(0);
  const pendingRef = useRef<PendingTransition | null>(null);
  const bootedRef = useRef(false);
  const animatedRef = useRef(false);

  const [booted, setBooted] = useState(false);
  const [layerOpen, setLayerOpen] = useState(false);
  const [settled, setSettled] = useState(false);
  const [opening, setOpening] = useState(false);
  const [transitionTick, setTransitionTick] = useState(0);
  const [headerDirection, setHeaderDirection] = useState(1);
  const [launching, setLaunching] = useState(false);
  const [creating, setCreating] = useState(false);
  const [generatingTopic, setGeneratingTopic] = useState("");

  const activeRoute = routes.find((route) => route.id === activeRouteId);
  const activeDetail = activeRouteId ? details[activeRouteId] : undefined;
  const activeNodes = activeDetail?.nodes ?? [];
  const activeIndex = activeNodes.findIndex((node) => node.id === activeNodeId);
  const activeNode = activeIndex >= 0 ? activeNodes[activeIndex] : undefined;
  const completedCount = activeNodes.filter((node) => node.status === "completed").length;
  const latestAdaptation = activeRouteId ? (adaptations[activeRouteId]?.[0] ?? null) : null;
  const showNotice = Boolean(
    latestAdaptation && activeRouteId && !noticeClosed[activeRouteId],
  );

  /* ---------------------------------------------------------------- bootstrap */

  useEffect(() => {
    if (bootedRef.current) return;
    bootedRef.current = true;
    void Promise.all([loadRoutes(), loadSources(), refreshInfo()]).then(() =>
      setBooted(true),
    );
  }, [loadRoutes, loadSources, refreshInfo]);

  /* --------------------------------------------------------------- view layer */

  useEffect(() => {
    if (!animatedRef.current) {
      animatedRef.current = true;
      return;
    }
    if (pendingRef.current || prefersReducedMotion()) return;
    const main = screenRef.current?.querySelector("main");
    if (!main?.animate) return;
    main.animate(
      [
        { opacity: 0, transform: `translateX(${direction * 24}px)` },
        { opacity: 1, transform: "translateX(-3px)", offset: 0.76 },
        { opacity: 1, transform: "translateX(0)" },
      ],
      { duration: 440, easing: "cubic-bezier(.22,.8,.3,1)" },
    );
  }, [view, direction]);

  // Restore the remembered scroll offset whenever a roadmap becomes visible.
  useLayoutEffect(() => {
    if (view !== "home") return;
    const home = homeScrollRef.current;
    if (!home) return;
    home.scrollTop = activeRouteId ? (scrollPositions[activeRouteId] ?? 0) : 0;
    scrollRef.current = home.scrollTop;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view, activeRouteId]);

  // A wrong answer, or a tutor that names a misconception, can insert a review
  // node; keep the roadmap and its notice strip honest.
  useEffect(() => {
    if (!learning.lastAdaptation || !activeRouteId) return;
    void loadDetail(activeRouteId);
    void loadAdaptations(activeRouteId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [learning.lastAdaptation?.id]);

  /* ------------------------------------------------------- lesson transition */

  useLayoutEffect(() => {
    const pending = pendingRef.current;
    if (!pending) return;
    pendingRef.current = null;
    const screen = screenRef.current;
    const root = screen?.parentElement;
    if (!screen || !root) return;

    if (pending.kind === "open") {
      const animations = animateLessonTransition(
        pending.underlay,
        screen,
        pending.clip,
        false,
      );
      void Promise.allSettled(animations.map((animation) => animation.finished)).then(
        () => {
          pending.underlay.remove();
          setOpening(false);
        },
      );
      return;
    }

    const home = screen.querySelector<HTMLElement>(".home-page");
    if (home && activeRouteId) home.scrollTop = scrollPositions[activeRouteId] ?? 0;
    const stop =
      (activeNodeId
        ? home?.querySelector<HTMLElement>(`.stop[data-node-id="${activeNodeId}"]`)
        : null) ?? home?.querySelector<HTMLElement>(".stop.active-stop");
    const bounds = stop?.getBoundingClientRect();
    const frame = root.getBoundingClientRect();
    if (!bounds) {
      pending.opening.remove();
      return;
    }
    const animations = animateLessonTransition(
      screen,
      pending.opening,
      lessonCardClip(bounds, frame),
      true,
    );
    void Promise.allSettled(animations.map((animation) => animation.finished)).then(
      () => pending.opening.remove(),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [transitionTick]);

  const openLesson = useCallback(
    (nodeId: string, trigger: HTMLElement) => {
      const screen = screenRef.current;
      const root = screen?.parentElement;
      if (!screen || !root || pendingRef.current || opening) return;
      setLayerOpen(false);
      setActiveNode(nodeId);
      void prepareLesson(nodeId);
      lessonScrollRef.current = 0;
      if (prefersReducedMotion()) {
        setView("lesson", 1);
        return;
      }
      const stop =
        trigger.closest(".stop") ??
        screen.querySelector<HTMLElement>(`.stop[data-node-id="${nodeId}"]`);
      const bounds = stop?.getBoundingClientRect();
      const frame = root.getBoundingClientRect();
      if (!bounds) {
        setView("lesson", 1);
        return;
      }
      const underlay = document.createElement("div");
      underlay.className = "lesson-underlay";
      underlay.setAttribute("inert", "");
      underlay.setAttribute("aria-hidden", "true");
      for (const child of Array.from(screen.childNodes)) {
        underlay.append(child.cloneNode(true));
      }
      const clonedHome = underlay.querySelector<HTMLElement>(".home-page");
      if (clonedHome) clonedHome.scrollTop = scrollRef.current;
      root.insertBefore(underlay, screen);
      pendingRef.current = {
        kind: "open",
        underlay,
        clip: lessonCardClip(bounds, frame),
      };
      setOpening(true);
      setView("lesson", 1);
      setTransitionTick((tick) => tick + 1);
    },
    [opening, prepareLesson, setActiveNode, setView],
  );

  const returnToRoadmap = useCallback(() => {
    const screen = screenRef.current;
    const root = screen?.parentElement;
    if (!screen || !root || pendingRef.current) return;
    if (activeRouteId) rememberScroll(activeRouteId, scrollRef.current);
    void learning.finish();
    setSettled(true);
    if (prefersReducedMotion()) {
      setView("home", -1);
      return;
    }
    const opening = document.createElement("div");
    opening.className = "lesson-opening";
    opening.setAttribute("inert", "");
    opening.setAttribute("aria-hidden", "true");
    for (const child of Array.from(screen.childNodes)) {
      opening.append(child.cloneNode(true));
    }
    opening.querySelectorAll("[id]").forEach((element) => element.removeAttribute("id"));
    root.append(opening);
    pendingRef.current = { kind: "close", opening };
    setView("home", -1);
    setTransitionTick((tick) => tick + 1);
  }, [activeRouteId, learning, rememberScroll, setView]);

  /* ------------------------------------------------------------ swipe engine */

  // Gesture callbacks read through this ref so the engine can stay bound for
  // the lifetime of the screen while still seeing the newest route / lesson.
  const latest = useRef({
    view,
    tab,
    routes,
    activeRouteId,
    details,
    lessons,
    activeNode,
    scrollPositions,
  });
  useLayoutEffect(() => {
    latest.current = {
      view,
      tab,
      routes,
      activeRouteId,
      details,
      lessons,
      activeNode,
      scrollPositions,
    };
  });

  const surfaceOf = useCallback(() => {
    const screen = screenRef.current;
    if (!screen) return null;
    return latest.current.view === "home"
      ? screen.querySelector<HTMLElement>(".home-page")
      : screen.querySelector<HTMLElement>(".lesson-content");
  }, []);

  const createPreview = useCallback(
    (dir: 1 | -1, paintedX: number): SwipePreview | null => {
      const screen = screenRef.current;
      const surface = surfaceOf();
      if (!screen || !surface) return null;
      const appBox = screen.getBoundingClientRect();
      const box = surface.getBoundingClientRect();
      const pane = surface.closest(".page") ?? screen;
      const paneBox = pane.getBoundingClientRect();
      const width = surface.offsetWidth || screen.clientWidth;
      const top = Math.max(box.top, paneBox.top, appBox.top);
      const bottom = Math.min(paneBox.bottom, appBox.bottom);
      if (bottom <= top) return null;
      const isHome = latest.current.view === "home";
      const gap = isHome ? 0 : Math.max(32, Math.min(48, width * 0.12));
      const travel = width + gap;

      const host = document.createElement("div");
      host.className = "swipe-preview-stage";
      host.setAttribute("inert", "");
      host.setAttribute("aria-hidden", "true");
      Object.assign(host.style, {
        left: `${box.left - paintedX - appBox.left}px`,
        top: `${top - appBox.top}px`,
        width: `${width}px`,
        height: `${bottom - top}px`,
      });

      const previewStyle: CSSProperties = {
        position: "absolute",
        top: `${box.top - top}px`,
        left: "0px",
        width: `${width}px`,
        height: isHome ? `${box.height}px` : "auto",
        maxHeight: "none",
        margin: "0",
        transform: `translate3d(${dir * travel + paintedX}px,0,0)`,
      };

      screen.append(host);

      const state = latest.current;
      let root: Root;
      let surfaceEl: HTMLElement | null;
      if (isHome) {
        const index = state.routes.findIndex((route) => route.id === state.activeRouteId);
        const target =
          state.routes[(index + dir + state.routes.length) % state.routes.length];
        if (!target) {
          host.remove();
          return null;
        }
        const mounted = mountPreview(host, () => (
          <RoadmapScreen
            route={target}
            detail={state.details[target.id]}
            preview
            settled
            style={previewStyle}
          />
        ));
        root = mounted.root;
        surfaceEl = mounted.surface;
        if (surfaceEl) {
          surfaceEl.scrollTop = latest.current.scrollPositions[target.id] ?? 0;
        }
      } else {
        const lesson = state.activeNode ? state.lessons[state.activeNode.id] : undefined;
        const targetTab = dir > 0 ? "materials" : "todo";
        const mounted = mountPreview(host, () =>
          lesson ? (
            <LessonPane
              lesson={lesson}
              node={state.activeNode}
              tab={targetTab}
              style={previewStyle}
            />
          ) : (
            <div className="lesson-content" style={previewStyle} />
          ),
        );
        root = mounted.root;
        surfaceEl = mounted.surface;
      }

      if (!surfaceEl) {
        root.unmount();
        host.remove();
        return null;
      }
      surfaceEl.classList.add("swipe-settled");
      const element = surfaceEl;

      return {
        host,
        surface: element,
        width,
        travel,
        gap,
        direction: dir,
        paint(x: number) {
          const progress = Math.min(1, Math.abs(x) / Math.max(1, width));
          const scale = 1 - Math.min(0.0035, progress * 0.0035);
          element.style.transform = `translate3d(${dir * travel + x}px,0,0) scale(${scale})`;
        },
        dispose() {
          root.unmount();
          host.remove();
        },
        adopt(commit) {
          // Swap React onto the destination first, then drop the stage: the two
          // are pixel-identical at this point, so no frame is ever blank.
          flushSync(() => commit());
          root.unmount();
          host.remove();
        },
      };
    },
    [surfaceOf],
  );

  const spec = useMemo<SwipeSpec>(
    () => ({
      surface: surfaceOf,
      zone: () => {
        const screen = screenRef.current;
        if (!screen) return null;
        return latest.current.view === "home"
          ? screen.querySelector<HTMLElement>(".home-page")
          : screen.querySelector<HTMLElement>("main.page");
      },
      frame: () => screenRef.current,
      canSwipe: (dir) => {
        const state = latest.current;
        if (state.view === "home") return state.routes.length > 1;
        if (state.view === "lesson") {
          return dir > 0 ? state.tab !== "materials" : state.tab !== "todo";
        }
        return false;
      },
      reducedMotion: () => prefersReducedMotion(),
      onDragStart: () => setOpening(false),
      createPreview: (dir, paintedX) => createPreview(dir, paintedX),
      commit: (dir) => {
        const state = latest.current;
        if (state.view === "home") {
          const index = state.routes.findIndex((route) => route.id === state.activeRouteId);
          if (index < 0) return;
          const next =
            state.routes[(index + dir + state.routes.length) % state.routes.length];
          if (!next) return;
          rememberScroll(state.activeRouteId ?? next.id, scrollRef.current);
          setHeaderDirection(dir);
          setSettled(true);
          setActiveRoute(next.id);
          return;
        }
        setTab(dir > 0 ? "materials" : "todo");
      },
    }),
    // Everything the gesture callbacks need is read from `latest.current`, and
    // the remaining entries are stable store actions / state setters, so the
    // listeners are bound once per view instead of once per render.
    [
      createPreview,
      surfaceOf,
      rememberScroll,
      setActiveRoute,
      setHeaderDirection,
      setSettled,
      setTab,
    ],
  );

  useEffect(() => {
    const zone = screenRef.current;
    if (!zone) return;
    return bindSwipeZone(zone, spec);
  }, [spec, booted]);

  /* ------------------------------------------------------------------ actions */

  const launchLearning = useCallback(
    (trigger: HTMLElement) => {
      if (launching || !activeNodeId) return;
      setLaunching(true);
      trigger.classList.add("launching");
      const go = async () => {
        const started = await learning.start(activeNodeId);
        setLaunching(false);
        trigger.classList.remove("launching");
        if (!started) {
          toast("这一课暂时打不开");
          return;
        }
        setView("learning", 1);
      };
      if (prefersReducedMotion()) void go();
      else window.setTimeout(() => void go(), 420);
    },
    [activeNodeId, launching, learning, setView],
  );

  const finishLesson = useCallback(async () => {
    if (!activeNodeId) return;
    await completeNode(activeNodeId);
    learning.reset();
    setSettled(true);
    setView("home", -1);
  }, [activeNodeId, completeNode, learning, setView]);

  const submitCreate = useCallback(
    async (input: RouteCreateFormInput) => {
      if (creating) return;
      setCreating(true);
      setGeneratingTopic(input.topic);
      const { collectWeb, ...request } = input;
      let sourceIds = request.sourceIds ?? [];
      if (collectWeb) {
        // Search and import before the job starts: the progress the learner
        // sees on the next screen is about the plan, not the crawl.
        const collected = await collectSources(input.topic, 6);
        sourceIds = [...sourceIds, ...collected];
        if (!collected.length) toast("没有找到可用的网页，先用你添加的资料");
      }
      const jobId = await useRouteStore
        .getState()
        .createRoute({ ...request, sourceIds });
      if (!jobId) {
        setCreating(false);
        toast("无法创建这条路线");
        return;
      }
      await startJob(jobId);
      setCreating(false);
      setView("generating", 1);
      const finished = await watchJob(jobId, (update) => applyJob(update));
      if (finished.status !== "completed") {
        toast("路线生成失败");
      }
    },
    [applyJob, collectSources, creating, setView, startJob],
  );

  const backToLayer = useCallback(() => {
    setView("home", -1);
    setLayerOpen(true);
  }, [setView]);

  const generationReady = useCallback(
    async (courseId: string | null) => {
      await loadRoutes();
      if (courseId) setActiveRoute(courseId);
      resetJob();
      setSettled(false);
      setView("home", -1);
      toast("学习路线已生成");
    },
    [loadRoutes, resetJob, setActiveRoute, setView],
  );

  /* -------------------------------------------------------------------- views */

  if (!booted || !activeRoute) {
    return (
      <div id="screen" ref={screenRef}>
        <main className="generation-page">
          <span className="generation-orbit">
            <Lighthouse height={64} className="pulse" />
          </span>
          <h1 className="page-title">正在打开学习空间</h1>
          <p>Lighthouse 正在为你点亮第一段路…</p>
        </main>
        <Toast />
      </div>
    );
  }

  const header = (
    <AppHeader
      title={activeRoute.title}
      onOpenLayer={() => setLayerOpen(true)}
      titleKey={`${activeRoute.id}:${view}`}
      direction={headerDirection}
      reduced={reduced}
    />
  );

  return (
    <div id="screen" ref={screenRef} className={opening ? "screen-opening" : undefined}>
      {view === "home" ? (
        <>
          {header}
          <RoadmapScreen
            route={activeRoute}
            detail={activeDetail}
            scrollRef={homeScrollRef}
            settled={settled}
            onOpenNode={openLesson}
            onScroll={(top) => {
              scrollRef.current = top;
            }}
          />
          {showNotice && latestAdaptation ? (
            <NoticeBanner
              adaptation={latestAdaptation}
              noticeKey={latestAdaptation.id}
              onDismiss={() => activeRouteId && dismissNotice(activeRouteId)}
            />
          ) : null}
        </>
      ) : null}

      {view === "lesson" ? (
        <LessonScreen
          route={activeRoute}
          node={activeNode}
          index={Math.max(0, activeIndex)}
          tab={tab}
          onBack={returnToRoadmap}
          onTab={setTab}
          onLaunch={launchLearning}
          reduced={reduced}
          launching={launching}
        />
      ) : null}

      {view === "learning" ? (
        <LearningScreen
          route={activeRoute}
          node={activeNode}
          onBackToLesson={() => setView("lesson", -1)}
          onHome={() => {
            void learning.finish();
            setSettled(true);
            setView("home", -1);
          }}
          onFinish={finishLesson}
          reduced={reduced}
        />
      ) : null}

      {view === "create" ? (
        <CreateRouteScreen
          onBack={backToLayer}
          onCreate={submitCreate}
          submitting={creating}
          searchConfigured={searchConfigured}
        />
      ) : null}

      {view === "generating" ? (
        <GeneratingScreen
          topic={generatingTopic}
          onCancel={() => {
            resetJob();
            setView("home", -1);
          }}
          onReady={generationReady}
        />
      ) : null}

      {view === "sources" ? <SourcesScreen onBack={backToLayer} /> : null}

      {view === "settings" ? <SettingsScreen onBack={backToLayer} /> : null}

      <RoadmapLayer
        open={layerOpen}
        routes={routes}
        activeRouteId={activeRouteId}
        completedCount={completedCount}
        nodeCount={activeNodes.length}
        onClose={() => setLayerOpen(false)}
        onSelect={(routeId) => {
          if (routeId === activeRouteId) return;
          setHeaderDirection(1);
          setSettled(false);
          setActiveRoute(routeId);
        }}
        onDelete={(routeId) => {
          void removeRoute(routeId);
          toast("路线已移除");
        }}
        onCreate={() => {
          setLayerOpen(false);
          setView("create", 1);
        }}
        onOpenSources={() => {
          setLayerOpen(false);
          setView("sources", 1);
        }}
        onOpenSettings={() => {
          setLayerOpen(false);
          setView("settings", 1);
        }}
        sourceCount={sources.length}
      />

      <Toast />
    </div>
  );
}
