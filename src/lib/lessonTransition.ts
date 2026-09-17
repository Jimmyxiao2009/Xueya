/**
 * The lesson card → fullscreen clip-path transition, and its exact reverse.
 *
 * Ported from the prototype: the tapped roadmap node's bounds become an
 * `inset(...)` clip on the incoming screen, while the outgoing screen dips
 * slightly behind it. Content inside then arrives in a short stagger, and the
 * whole thing plays backwards when the learner returns to the roadmap.
 */

interface Box {
  top: number;
  left: number;
  right: number;
  bottom: number;
  width: number;
  height: number;
}

export function lessonCardClip(bounds: Box, frame: Box): string {
  const top = Math.max(0, Math.min(frame.height - 48, bounds.top - frame.top - 6));
  const bottom = Math.max(
    0,
    frame.height - Math.max(top + 48, Math.min(frame.height, bounds.bottom - frame.top + 6)),
  );
  const left = Math.max(0, Math.min(frame.width - 48, bounds.left - frame.left - 8));
  const right = Math.max(
    0,
    frame.width - Math.max(left + 48, Math.min(frame.width, bounds.right - frame.left + 8)),
  );
  return `inset(${top}px ${right}px ${bottom}px ${left}px round 24px)`;
}

interface Track {
  element: HTMLElement;
  frames: Keyframe[];
  duration: number;
  delay: number;
  easing: string;
}

export function animateLessonTransition(
  from: HTMLElement,
  to: HTMLElement,
  clip: string,
  reverse = false,
): Animation[] {
  // jsdom (and any browser without WAAPI) skips straight to the end state: the
  // screen is already mounted, so the transition is simply not played.
  if (!from.animate || !to.animate) return [];
  const tracks: Track[] = [];
  const track = (
    element: HTMLElement | null | undefined,
    frames: Keyframe[],
    duration: number,
    delay = 0,
    easing = "cubic-bezier(.2,.8,.2,1)",
  ) => {
    if (element?.animate) tracks.push({ element, frames, duration, delay, easing });
  };

  track(
    from,
    [
      { opacity: 1, transform: "scale(1)" },
      { opacity: 0.42, transform: "scale(.978)" },
    ],
    390,
  );
  track(
    to,
    [{ clipPath: clip }, { clipPath: "inset(0px 0px 0px 0px round 0px)" }],
    470,
    0,
    "cubic-bezier(.18,.9,.22,1)",
  );
  track(
    to.querySelector<HTMLElement>(".compact-top"),
    [
      { opacity: 0, transform: "translateY(-6px)" },
      { opacity: 1, transform: "translateY(0)" },
    ],
    210,
    95,
  );

  const groups: (HTMLElement | null)[] = [
    to.querySelector<HTMLElement>(".back"),
    to.querySelector<HTMLElement>(".course-top"),
    to.querySelector<HTMLElement>(".pills"),
    to.querySelector<HTMLElement>(".tabs"),
    to.querySelector<HTMLElement>(".section-label"),
    ...Array.from(to.querySelectorAll<HTMLElement>(".todo,.source-card")),
    to.querySelector<HTMLElement>(".swipe-tip"),
  ];
  groups.forEach((element, index) =>
    track(
      element,
      [
        { opacity: 0, transform: "translateY(12px)" },
        { opacity: 1, transform: "translateY(0)" },
      ],
      225,
      82 + index * 18,
    ),
  );
  track(
    to.querySelector<HTMLElement>(".lesson-dock"),
    [
      { opacity: 0, transform: "translateY(28px) scale(.985)" },
      { opacity: 1, transform: "translateY(0) scale(1)" },
    ],
    280,
    165,
  );

  const length = Math.max(...tracks.map((entry) => entry.delay + entry.duration));
  const animations = tracks.map((entry) =>
    entry.element.animate(entry.frames, {
      duration: entry.duration,
      delay: reverse ? length - entry.delay - entry.duration : entry.delay,
      easing: entry.easing,
      direction: reverse ? "reverse" : "normal",
      fill: "both",
    }),
  );
  const start = document.timeline?.currentTime;
  if (start != null) {
    animations.forEach((animation) => {
      animation.startTime = start;
    });
  }
  return animations;
}
