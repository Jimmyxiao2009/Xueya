import { useEffect } from "react";
import { clicksAreSuppressed } from "@/lib/swipeEngine";
import { prefersReducedMotion } from "@/hooks/useReducedMotion";

/**
 * Press / release feedback for every control in the app.
 *
 * The prototype replaced a view *after* the release animation had a chance to
 * be seen, by intercepting the click in the capture phase and re-dispatching it
 * a few milliseconds later. Buttons can opt out with `data-motion="own"`
 * (launch / continue own their transition) or shorten the hold with
 * `data-motion="menu"`.
 */

const motion = new WeakMap<HTMLElement, Animation>();
const releaseTimes = new WeakMap<HTMLElement, number>();
const pressedPointers = new Map<number, HTMLElement>();
const queued = new WeakSet<HTMLElement>();
const forwarded = new WeakSet<HTMLElement>();

export function cancelPress(button: HTMLElement): void {
  motion.get(button)?.cancel();
  motion.delete(button);
}

function faceOf(button: HTMLElement): HTMLElement {
  return (
    (button.querySelector(".node,.info-face") as HTMLElement | null) ?? button
  );
}

function animateControl(button: HTMLElement, releasing = false, pulse = false): void {
  if (!button || (button as HTMLButtonElement).disabled || prefersReducedMotion()) return;
  const face = faceOf(button);
  if (!face.animate) return;
  motion.get(button)?.cancel();
  const scale = Number.parseFloat(getComputedStyle(face).scale) || 1;
  const pressed = button.matches(".node-hit,.primary,.send") ? 0.91 : 0.965;
  const frames: Keyframe[] = releasing
    ? [
        { scale: pulse ? 1 : scale, offset: 0 },
        ...(pulse ? [{ scale: pressed, offset: 0.18 }] : []),
        { scale: 1.055, offset: 0.53 },
        { scale: 0.987, offset: 0.72 },
        { scale: 1, offset: 1 },
      ]
    : [{ scale }, { scale: pressed }];
  const animation = face.animate(frames, {
    duration: releasing ? 340 : 85,
    easing: releasing ? "cubic-bezier(.2,.65,.35,1)" : "ease-out",
    fill: "forwards",
  });
  motion.set(button, animation);
  if (releasing) {
    releaseTimes.set(button, performance.now());
    animation.finished
      .then(() => {
        if (motion.get(button) === animation) {
          animation.cancel();
          motion.delete(button);
        }
      })
      .catch(() => undefined);
  }
}

export function useControlFeedback(): void {
  useEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest?.("#learning-plan")) {
        window.dispatchEvent(new CustomEvent("orbit:plan-touch"));
      }
      const button = target?.closest?.("button") as HTMLElement | null;
      if (!button || (button as HTMLButtonElement).disabled || event.button > 0) return;
      pressedPointers.set(event.pointerId, button);
      animateControl(button);
    };

    const onPointerUp = (event: PointerEvent) => {
      const button = pressedPointers.get(event.pointerId);
      pressedPointers.delete(event.pointerId);
      if (button) animateControl(button, true);
    };

    const onPointerCancel = (event: PointerEvent) => {
      const button = pressedPointers.get(event.pointerId);
      pressedPointers.delete(event.pointerId);
      if (button) cancelPress(button);
    };

    const onBlur = () => {
      pressedPointers.forEach((button) => cancelPress(button));
      pressedPointers.clear();
    };

    const onClick = (event: MouseEvent) => {
      if (clicksAreSuppressed()) {
        event.preventDefault();
        event.stopImmediatePropagation();
        return;
      }
      const button = (event.target as HTMLElement | null)?.closest?.(
        "button",
      ) as HTMLElement | null;
      if (!button || (button as HTMLButtonElement).disabled) return;
      if (forwarded.has(button)) {
        forwarded.delete(button);
        return;
      }
      if (prefersReducedMotion()) return;
      if (performance.now() - (releaseTimes.get(button) ?? -Infinity) > 150) {
        animateControl(button, true, true);
      }
      if (button.dataset.motion === "own") return;
      event.preventDefault();
      event.stopImmediatePropagation();
      if (queued.has(button)) return;
      queued.add(button);
      window.setTimeout(
        () => {
          queued.delete(button);
          if (!button.isConnected || (button as HTMLButtonElement).disabled) return;
          if (clicksAreSuppressed()) return;
          forwarded.add(button);
          button.click();
        },
        button.dataset.motion === "menu" ? 36 : 52,
      );
    };

    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("pointerup", onPointerUp);
    document.addEventListener("pointercancel", onPointerCancel);
    document.addEventListener("click", onClick, true);
    window.addEventListener("blur", onBlur);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("pointerup", onPointerUp);
      document.removeEventListener("pointercancel", onPointerCancel);
      document.removeEventListener("click", onClick, true);
      window.removeEventListener("blur", onBlur);
    };
  }, []);
}

/** Any control the drag engine should cancel press feedback for. */
export function cancelAllPress(): void {
  pressedPointers.forEach((button) => cancelPress(button));
  pressedPointers.clear();
}
