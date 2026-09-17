import { useEffect } from "react";

/**
 * The ambient roadmap motion.
 *
 * Node and label buoys drift on slow, overlapping sine waves — position and
 * roll follow different phases so the path never looks like a single loop.
 * Frames are precomputed once and handed to the compositor, which keeps the
 * hit targets still and avoids a JS frame loop.
 *
 * The prototype deliberately keeps this running even under
 * `prefers-reduced-motion`: it is the product's identity, and it is gentle
 * enough not to be an accessibility hazard. It pauses while the management
 * layer is open.
 */
const FRAMES = 241;
const frameCache = new Map<string, Keyframe[]>();

function framesFor(index: number, part: number): Keyframe[] {
  const key = `${index}:${part}`;
  const cached = frameCache.get(key);
  if (cached) return cached;
  const phase = index * 0.81;
  const amplitude = 0.91 + (index % 3) * 0.055;
  const frames: Keyframe[] = Array.from({ length: FRAMES }, (_, frame) => {
    const t = (frame / (FRAMES - 1)) * Math.PI * 2;
    const x = 1.2 * Math.sin(2 * t + phase + 0.8) + 0.45 * Math.sin(5 * t - phase);
    const y =
      amplitude *
      (4.2 * Math.sin(4 * t + phase) +
        0.85 * Math.sin(7 * t + phase * 1.4 + 0.9) +
        0.3 * Math.sin(11 * t - phase));
    const roll =
      (1.05 * Math.cos(4 * t + phase + 0.55) + 0.25 * Math.sin(7 * t + phase + 1.6)) *
      (part ? 0.3 : 1);
    return {
      transform: `translate3d(${x.toFixed(3)}px,${y.toFixed(3)}px,0) rotate(${roll.toFixed(3)}deg)`,
    };
  });
  frameCache.set(key, frames);
  return frames;
}

export interface AmbientOptions {
  /** Scrolling container used as the IntersectionObserver root. */
  root: () => HTMLElement | null;
  /** When true the waves pause (the roadmap layer is open). */
  paused: () => boolean;
}

/** Every live wave track, so the management layer can pause the whole field. */
const liveTracks = new Set<Animation>();
let pausedByLayer = false;

export function pauseAmbientRoadmap(): void {
  pausedByLayer = true;
  liveTracks.forEach((track) => track.pause());
}

export function resumeAmbientRoadmap(): void {
  pausedByLayer = false;
  liveTracks.forEach((track) => {
    if (track.effect?.getComputedTiming().progress !== null) track.play();
  });
}

export function startAmbientRoadmap(container: HTMLElement, options: AmbientOptions): () => void {
  const tracks: Animation[] = [];
  const visibility = new Map<Element, Animation[]>();

  container.querySelectorAll(".stop").forEach((stop, index) => {
    const owned: Animation[] = [];
    [".node-buoy", ".label-buoy"].forEach((selector, part) => {
      const element = stop.querySelector(selector) as HTMLElement | null;
      if (!element?.animate) return;
      const track = element.animate(framesFor(index, part), {
        duration: 36000,
        iterations: Infinity,
        easing: "linear",
        fill: "both",
      });
      owned.push(track);
      tracks.push(track);
      liveTracks.add(track);
    });
    visibility.set(stop, owned);
  });

  let observer: IntersectionObserver | null = null;
  if (typeof IntersectionObserver === "function") {
    observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          visibility.get(entry.target)?.forEach((track) => {
            if (entry.isIntersecting && !options.paused() && !pausedByLayer) track.play();
            else track.pause();
          });
        });
      },
      { root: options.root(), rootMargin: "80px 0px" },
    );
    visibility.forEach((_, stop) => observer?.observe(stop));
  }

  return () => {
    observer?.disconnect();
    tracks.forEach((track) => {
      track.cancel();
      liveTracks.delete(track);
    });
    tracks.length = 0;
    visibility.clear();
  };
}

export function useAmbientRoadmap(
  container: React.RefObject<HTMLElement | null>,
  options: AmbientOptions,
  deps: unknown[] = [],
): void {
  useEffect(() => {
    const element = container.current;
    if (!element) return;
    const stop = startAmbientRoadmap(element, options);
    return stop;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}
