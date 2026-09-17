import { useEffect } from "react";

/**
 * The prototype measured the *visual* viewport, not the layout viewport, so the
 * device frame keeps its height when a mobile keyboard opens and the composer
 * lifts above it.
 */
export function useVisualViewport(): void {
  useEffect(() => {
    const sync = () => {
      const viewport = window.visualViewport;
      const height = viewport?.height || window.innerHeight;
      const root = document.documentElement;
      root.style.setProperty("--app-height", `${height}px`);
      const inset = window.innerHeight - (viewport?.height ?? window.innerHeight) - (viewport?.offsetTop ?? 0);
      root.style.setProperty("--keyboard-inset", `${Math.max(0, Math.round(inset))}px`);
    };
    sync();
    window.visualViewport?.addEventListener("resize", sync);
    window.visualViewport?.addEventListener("scroll", sync);
    window.addEventListener("resize", sync);
    return () => {
      window.visualViewport?.removeEventListener("resize", sync);
      window.visualViewport?.removeEventListener("scroll", sync);
      window.removeEventListener("resize", sync);
    };
  }, []);
}
