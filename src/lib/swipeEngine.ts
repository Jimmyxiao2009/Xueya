/**
 * Orbit's weighted swipe engine.
 *
 * Ported from `reference/Orbit_motion_continuous_swipe.html` with the same
 * numbers: finger-following drag with two soft detents, velocity-aware commit,
 * a live preview of the adjacent page, and a spring settle in both directions.
 *
 * It is deliberately imperative and framework-free: the gesture loop must not
 * re-render React on every pointer move.
 */

export interface SwipePreview {
  /** Positioned stage that hosts the adjacent page. */
  host: HTMLElement;
  /** The moving page inside the stage. */
  surface: HTMLElement;
  width: number;
  travel: number;
  gap: number;
  direction: 1 | -1;
  paint: (x: number) => void;
  dispose: () => void;
  /** Hand the preview over to React: the caller commits, we clean up. */
  adopt: (commit: () => void) => void;
}

export interface SwipeSpec {
  /** Element that follows the finger. */
  surface: () => HTMLElement | null;
  /** Element that receives `is-dragging`. */
  zone: () => HTMLElement | null;
  /** Frame used for widths and for positioning the preview stage. */
  frame: () => HTMLElement | null;
  canSwipe: (direction: 1 | -1) => boolean;
  createPreview: (direction: 1 | -1, paintedX: number) => SwipePreview | null;
  commit: (direction: 1 | -1, preview: SwipePreview | null) => void;
  reducedMotion: () => boolean;
  /** Called once a drag is recognised, so press feedback can be cancelled. */
  onDragStart?: () => void;
}

/** Soft detents: the page gives way, then resists, then gives way again. */
export function detailDrag(
  raw: number,
  width: number,
): { offset: number; slope: number } {
  const sign = Math.sign(raw) || 1;
  const d = Math.abs(raw);
  const lead = 18;
  const detentStart = width * 0.31;
  const detentEnd = width * 0.43;
  const cap = width * 0.94;
  let x: number;
  if (d <= lead) {
    x = d;
  } else {
    const q = d - lead;
    x = lead + q / (1 + q / (width * 1.85));
  }
  if (d > detentStart) {
    const q0 = Math.max(0, detentStart - lead);
    const before = lead + q0 / (1 + q0 / (width * 1.85));
    if (d < detentEnd) {
      const q = Math.max(0, d - lead);
      const baseAtD = lead + q / (1 + q / (width * 1.85));
      x = before + (baseAtD - before) * 0.48;
    } else {
      const qe = Math.max(0, detentEnd - lead);
      const baseEnd = lead + qe / (1 + qe / (width * 1.85));
      const heldEnd = before + (baseEnd - before) * 0.48;
      x = heldEnd + (d - detentEnd) * 0.74;
    }
  }
  x = Math.min(x, cap);
  const slope = d < lead ? 1 : d < detentStart ? 0.82 : d < detentEnd ? 0.4 : 0.7;
  return { offset: sign * x, slope };
}

export function swipeTiming(
  distance: number,
  releaseSpeed: number | null = null,
): KeyframeAnimationOptions {
  if (releaseSpeed === null) {
    return { duration: 720, easing: "cubic-bezier(.14,.68,.16,1)", fill: "both" };
  }
  const speed = Math.max(0, releaseSpeed);
  const duration = Math.max(560, Math.min(1000, 820 + distance * 0.24 - speed * 140));
  return { duration, easing: "cubic-bezier(.10,.64,.14,1)", fill: "both" };
}

let swipeBusy = false;
let activeTransition: { finish: () => void } | null = null;
let suppressClicksUntil = 0;

export function isSwipeBusy(): boolean {
  return swipeBusy;
}

export function suppressClicksFor(ms: number): void {
  suppressClicksUntil = performance.now() + ms;
}

export function clicksAreSuppressed(): boolean {
  return swipeBusy || performance.now() < suppressClicksUntil;
}

/** Complete an in-flight settle synchronously so a new touch can take over. */
export function finishActiveSwipe(): void {
  activeTransition?.finish();
}

/**
 * Drop every piece of module-level gesture state.
 *
 * The engine is deliberately framework-free and singleton, so a zone that
 * unmounts mid-gesture (or a test that starts from scratch) has to be able to
 * hand the screen back in a clean state — otherwise `swipeBusy` would keep
 * swallowing every click forever.
 */
export function resetSwipeEngine(): void {
  activeTransition?.finish();
  activeTransition = null;
  swipeBusy = false;
  suppressClicksUntil = 0;
  document.documentElement.classList.remove("motion-lite");
}

type PointerId = number | string;

interface Gesture {
  id: PointerId;
  x: number;
  y: number;
  dx: number;
  horizontal: boolean;
  surface: HTMLElement;
  samples: { x: number; t: number }[];
  preview: SwipePreview | null;
  paintedX: number;
  dragSlope: number;
}

export function bindSwipeZone(zone: HTMLElement, spec: SwipeSpec): () => void {
  let gesture: Gesture | null = null;
  let dragFrame: number | null = null;
  let wheelGesture: WheelGesture | null = null;
  let wheelTimer: number | undefined;
  let wheelQuietUntil = 0;

  interface WheelGesture {
    surface: HTMLElement;
    dx: number;
    samples: { x: number; t: number }[];
    velocity: number;
    preview: SwipePreview | null;
    paintedX: number;
  }

  const frameWidth = () => spec.frame()?.clientWidth || window.innerWidth;

  const position = (g: Gesture | WheelGesture) => {
    const x = g.paintedX;
    const surface = g.surface;
    if (!surface) return;
    const width = g.preview?.width || frameWidth();
    const p = Math.min(1, Math.abs(x) / Math.max(1, width));
    const scale = 1 - Math.min(0.0045, p * 0.0045);
    surface.style.transform = `translate3d(${x}px,0,0) scale(${scale})`;
    g.preview?.paint(x);
  };

  const paintDrag = (g: Gesture | WheelGesture) => {
    if (dragFrame !== null) return;
    dragFrame = requestAnimationFrame(() => {
      dragFrame = null;
      if (gesture === g || wheelGesture === g) position(g);
    });
  };

  const flushDrag = (g: Gesture | WheelGesture) => {
    if (dragFrame !== null) {
      cancelAnimationFrame(dragFrame);
      dragFrame = null;
    }
    position(g);
  };

  const prepare = (
    g: Gesture | WheelGesture,
    dx: number,
    available: boolean,
  ) => {
    const direction: 1 | -1 = dx < 0 ? 1 : -1;
    if (g.preview && (!available || g.preview.direction !== direction)) {
      g.preview.dispose();
      g.preview = null;
    }
    if (available && !g.preview) {
      g.preview = spec.createPreview(direction, g.paintedX || 0);
    }
  };

  const record = (g: Gesture | WheelGesture, x: number, now: number) => {
    g.samples.push({ x, t: now });
    while (g.samples.length > 2 && g.samples[1].t < now - 90) g.samples.shift();
  };

  const speedAt = (g: Gesture | WheelGesture, x: number, now: number) => {
    const first = g.samples.find((sample) => now - sample.t <= 100);
    return first ? (x - first.x) / Math.max(1, now - first.t) : 0;
  };

  const reset = () => {
    if (!gesture) return;
    const g = gesture;
    flushDrag(g);
    gesture = null;
    zone.classList.remove("is-dragging");
    if (g.horizontal) settleBack(spec, g.surface, g.preview, g.dx);
    else g.preview?.dispose();
  };

  const available = (dx: number) => spec.canSwipe(dx < 0 ? 1 : -1);

  const begin = (event: {
    pointerId: PointerId;
    clientX: number;
    clientY: number;
    button: number;
    isPrimary?: boolean;
    target: EventTarget | null;
  }) => {
    if (event.button > 0 || event.isPrimary === false) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest?.("input,textarea")) return;
    if (swipeBusy) activeTransition?.finish();
    if (swipeBusy) return;
    const surface = spec.surface();
    if (!surface) return;
    gesture = {
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      dx: 0,
      horizontal: false,
      surface,
      samples: [{ x: 0, t: performance.now() }],
      preview: null,
      paintedX: 0,
      dragSlope: 1,
    };
  };

  const move = (event: {
    pointerId: PointerId;
    clientX: number;
    clientY: number;
    fromTouch?: boolean;
    preventDefault: () => void;
  }) => {
    const g = gesture;
    if (!g || g.id !== event.pointerId) return;
    const dx = event.clientX - g.x;
    const dy = event.clientY - g.y;
    if (!g.horizontal) {
      if (Math.abs(dy) > 10 && Math.abs(dy) > Math.abs(dx)) {
        gesture = null;
        return;
      }
      if (Math.abs(dx) < 10 || Math.abs(dx) < Math.abs(dy) * 1.25) return;
      g.horizontal = true;
      zone.classList.add("is-dragging");
      if (!event.fromTouch) {
        try {
          zone.setPointerCapture?.(event.pointerId as number);
        } catch {
          /* capture is an optimisation, not a requirement */
        }
      }
      spec.onDragStart?.();
    }
    event.preventDefault();
    const isAvailable = available(dx);
    prepare(g, dx, isAvailable);
    record(g, dx, performance.now());
    const damped = isAvailable ? detailDrag(dx, g.preview?.width || frameWidth()) : null;
    g.dragSlope = damped?.slope ?? 1;
    g.dx = damped ? damped.offset : dx * 0.18;
    g.paintedX = g.dx;
    paintDrag(g);
  };

  const end = (event: { pointerId: PointerId; clientX: number }) => {
    const g = gesture;
    if (!g || g.id !== event.pointerId) return;
    if (!g.horizontal) {
      gesture = null;
      return;
    }
    suppressClicksUntil = performance.now() + 90;
    const raw = event.clientX - g.x;
    const velocity = speedAt(g, raw, performance.now());
    const isAvailable = available(raw);
    const commitDistance = Math.min(132, frameWidth() * 0.335);
    const fastFlick = Math.abs(raw) > 26 && Math.abs(velocity) > 0.58;
    if (!isAvailable || !(Math.abs(raw) > commitDistance || fastFlick)) {
      reset();
      return;
    }
    flushDrag(g);
    gesture = null;
    zone.classList.remove("is-dragging");
    const direction: 1 | -1 = raw < 0 ? 1 : -1;
    slideAcross(spec, g.surface, direction, g.dx, velocity * g.dragSlope, g.preview, (preview) =>
      spec.commit(direction, preview),
    );
  };

  const onPointerDown = (event: PointerEvent) => {
    if (event.pointerType !== "touch") begin(event);
  };
  const onPointerMove = (event: PointerEvent) => {
    if (event.pointerType !== "touch") move(event);
  };
  const onPointerUp = (event: PointerEvent) => {
    if (event.pointerType !== "touch") end(event);
  };
  const onPointerCancel = () => reset();

  const touchEvent = (event: TouchEvent, touch: Touch) => ({
    pointerId: `touch-${touch.identifier}`,
    clientX: touch.clientX,
    clientY: touch.clientY,
    button: 0,
    target: event.target,
    fromTouch: true,
    preventDefault: () => {
      if (event.cancelable) event.preventDefault();
    },
  });

  const onTouchStart = (event: TouchEvent) => {
    if (event.touches.length === 1) begin(touchEvent(event, event.touches[0]));
    else reset();
  };
  const onTouchMove = (event: TouchEvent) => {
    const touch = Array.from(event.touches).find(
      (candidate) => `touch-${candidate.identifier}` === gesture?.id,
    );
    if (touch) move(touchEvent(event, touch));
  };
  const onTouchEnd = (event: TouchEvent) => {
    const touch = Array.from(event.changedTouches).find(
      (candidate) => `touch-${candidate.identifier}` === gesture?.id,
    );
    if (touch) end(touchEvent(event, touch));
  };

  const onWheel = (event: WheelEvent) => {
    if (event.ctrlKey) return;
    const raw = event.deltaX || (event.shiftKey ? event.deltaY : 0);
    if (!raw || (!event.shiftKey && Math.abs(raw) <= Math.abs(event.deltaY) * 1.15)) return;
    event.preventDefault();
    const now = performance.now();
    if (now < wheelQuietUntil) {
      wheelQuietUntil = Math.max(wheelQuietUntil, now + 70);
      return;
    }
    if (swipeBusy) activeTransition?.finish();
    if (swipeBusy || gesture) return;
    const surface = spec.surface();
    if (!surface) return;
    if (!wheelGesture) {
      wheelGesture = {
        surface,
        dx: 0,
        samples: [{ x: 0, t: now }],
        velocity: 0,
        preview: null,
        paintedX: 0,
      };
    }
    const g = wheelGesture;
    const factor = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? frameWidth() : 1;
    g.dx = Math.max(-frameWidth() * 0.8, Math.min(frameWidth() * 0.8, g.dx - raw * factor));
    const isAvailable = available(g.dx);
    prepare(g, g.dx, isAvailable);
    const damped = isAvailable ? detailDrag(g.dx, g.preview?.width || frameWidth()) : null;
    const visual = damped ? damped.offset : g.dx * 0.18;
    record(g, g.dx, now);
    g.velocity = Math.max(-3, Math.min(3, speedAt(g, g.dx, now))) * (damped?.slope ?? 1);
    g.paintedX = visual;
    paintDrag(g);
    window.clearTimeout(wheelTimer);
    wheelTimer = window.setTimeout(() => {
      flushDrag(g);
      const finished = g;
      wheelGesture = null;
      if (!finished.surface.isConnected) return;
      suppressClicksUntil = performance.now() + 80;
      wheelQuietUntil = performance.now() + 80;
      if (!isAvailable || Math.abs(finished.dx) < Math.min(88, frameWidth() * 0.22)) {
        settleBack(spec, finished.surface, finished.preview, visual);
        return;
      }
      const direction: 1 | -1 = finished.dx < 0 ? 1 : -1;
      slideAcross(
        spec,
        finished.surface,
        direction,
        visual,
        finished.velocity,
        finished.preview,
        (preview) => spec.commit(direction, preview),
      );
    }, 40);
  };

  const onLostCapture = (event: PointerEvent) => {
    if (event.target === zone && gesture?.id === event.pointerId) reset();
  };

  zone.addEventListener("pointerdown", onPointerDown);
  zone.addEventListener("pointermove", onPointerMove);
  zone.addEventListener("pointerup", onPointerUp);
  zone.addEventListener("pointercancel", onPointerCancel);
  zone.addEventListener("touchstart", onTouchStart, { passive: true });
  zone.addEventListener("touchmove", onTouchMove, { passive: false });
  zone.addEventListener("touchend", onTouchEnd, { passive: true });
  zone.addEventListener("touchcancel", reset, { passive: true });
  zone.addEventListener("lostpointercapture", onLostCapture as EventListener);
  zone.addEventListener("wheel", onWheel, { passive: false });

  return () => {
    zone.removeEventListener("pointerdown", onPointerDown);
    zone.removeEventListener("pointermove", onPointerMove);
    zone.removeEventListener("pointerup", onPointerUp);
    zone.removeEventListener("pointercancel", onPointerCancel);
    zone.removeEventListener("touchstart", onTouchStart);
    zone.removeEventListener("touchmove", onTouchMove);
    zone.removeEventListener("touchend", onTouchEnd);
    zone.removeEventListener("touchcancel", reset);
    zone.removeEventListener("lostpointercapture", onLostCapture as EventListener);
    zone.removeEventListener("wheel", onWheel);
    window.clearTimeout(wheelTimer);
    if (dragFrame !== null) cancelAnimationFrame(dragFrame);
    gesture = null;
    wheelGesture = null;
    resetSwipeEngine();
  };
}

/** Animate back to rest after a drag that did not commit. */
export async function settleBack(
  spec: SwipeSpec,
  surface: HTMLElement,
  preview: SwipePreview | null,
  startX: number,
): Promise<void> {
  if (!surface.isConnected) {
    preview?.dispose();
    return;
  }
  // No WAAPI (older WebView, jsdom): snap back instead of animating.
  if (!surface.animate || spec.reducedMotion()) {
    surface.style.transform = "";
    preview?.dispose();
    return;
  }
  swipeBusy = true;
  const animations: Animation[] = [];
  const distance = Math.abs(startX);
  const timing: KeyframeAnimationOptions = {
    duration: Math.max(480, Math.min(720, 470 + distance * 0.96)),
    easing: "cubic-bezier(.14,.66,.18,1)",
    fill: "both",
  };
  const recoil = Math.sign(startX) * Math.min(4, Math.max(1.5, distance * 0.025));
  let finished = false;
  const finish = () => {
    if (finished) return;
    finished = true;
    animations.forEach((animation) => {
      try {
        animation.cancel();
      } catch {
        /* already cancelled */
      }
    });
    surface.style.transform = "";
    preview?.dispose();
    swipeBusy = false;
    if (activeTransition?.finish === finish) activeTransition = null;
  };
  activeTransition = { finish };
  try {
    animations.push(
      surface.animate(
        [
          { transform: `translate3d(${startX}px,0,0) scale(.998)` },
          { transform: `translate3d(${-recoil}px,0,0) scale(.9995)`, offset: 0.86 },
          { transform: "translate3d(0,0,0) scale(1)" },
        ],
        timing,
      ),
    );
    if (preview) {
      animations.push(
        preview.surface.animate(
          [
            {
              transform: `translate3d(${preview.direction * preview.travel + startX}px,0,0) scale(.998)`,
            },
            {
              transform: `translate3d(${preview.direction * preview.travel - recoil}px,0,0) scale(.9995)`,
              offset: 0.86,
            },
            {
              transform: `translate3d(${preview.direction * preview.travel}px,0,0) scale(1)`,
            },
          ],
          timing,
        ),
      );
    }
    await Promise.allSettled(animations.map((animation) => animation.finished));
  } finally {
    finish();
  }
}

/** Animate the two pages past each other and adopt the incoming one. */
export async function slideAcross(
  spec: SwipeSpec,
  surface: HTMLElement,
  direction: 1 | -1,
  startX: number,
  releaseVelocity: number | null,
  prepared: SwipePreview | null,
  commit: (preview: SwipePreview | null) => void,
): Promise<void> {
  if (swipeBusy) {
    prepared?.dispose();
    return;
  }
  if (!surface.animate || spec.reducedMotion()) {
    if (prepared) prepared.adopt(() => commit(prepared));
    else commit(null);
    return;
  }
  const preview = prepared ?? spec.createPreview(direction, startX);
  if (!preview) {
    surface.style.transform = "";
    commit(null);
    return;
  }
  swipeBusy = true;
  document.documentElement.classList.add("motion-lite");
  surface.classList.add("swipe-settled");
  const animations: Animation[] = [];
  let finished = false;
  let committed = false;
  const finish = () => {
    if (finished) return;
    finished = true;
    animations.forEach((animation) => {
      try {
        animation.cancel();
      } catch {
        /* already cancelled */
      }
    });
    surface.style.transform = "";
    surface.classList.remove("swipe-landing");
    preview.surface?.classList?.remove("swipe-landing");
    if (surface.isConnected && !committed) {
      committed = true;
      preview.adopt(() => commit(preview));
    } else {
      preview.dispose();
    }
    swipeBusy = false;
    document.documentElement.classList.remove("motion-lite");
    if (activeTransition?.finish === finish) activeTransition = null;
  };
  activeTransition = { finish };
  try {
    const timing = swipeTiming(
      Math.max(1, preview.travel - Math.abs(startX)),
      releaseVelocity === null ? null : Math.max(0, -direction * releaseVelocity),
    );
    const seat = Math.min(9, Math.max(5, preview.width * 0.016));
    surface.classList.add("swipe-landing");
    preview.surface.classList.add("swipe-landing");
    animations.push(
      surface.animate(
        [
          { transform: `translate3d(${startX}px,0,0) scale(.998)` },
          {
            transform: `translate3d(${-direction * (preview.travel + seat)}px,0,0) scale(.997)`,
            offset: 0.72,
          },
          { transform: `translate3d(${-direction * preview.travel}px,0,0) scale(1)` },
        ],
        timing,
      ),
    );
    animations.push(
      preview.surface.animate(
        [
          {
            transform: `translate3d(${direction * preview.travel + startX}px,0,0) scale(.998)`,
          },
          { transform: `translate3d(${-direction * seat}px,0,0) scale(.999)`, offset: 0.72 },
          { transform: "translate3d(0,0,0) scale(1)" },
        ],
        timing,
      ),
    );
    const start = document.timeline?.currentTime;
    if (start != null) {
      animations.forEach((animation) => {
        animation.startTime = start;
      });
    }
    await Promise.allSettled(animations.map((animation) => animation.finished));
  } finally {
    finish();
  }
}
