import { LighthouseBeacon } from "./Lighthouse";
import { useThemeStore } from "@/stores/themeStore";

/** Shared visual adapter; color and mastery are owned by semantic CSS tokens. */
export function NavigationAnchor({ size = 52, className = "" }: { size?: number; className?: string }) {
  const motif = useThemeStore((state) => state.iconMotif);
  return <span className={`navigation-anchor ${className}`} aria-hidden="true" data-anchor={motif}>
    <span className="navigation-halo" />
    {motif === "lighthouse" ? <LighthouseBeacon state="lit" primary height={size} /> : <svg width={size} height={size} viewBox="0 0 64 64" fill="none">
      <path d="M32 7v10M25 17h14l4 29H21z" fill="var(--world-surface-elevated)" stroke="currentColor" strokeWidth="2" />
      <path d="M24 27h16l1 8H23z" fill="var(--beacon-core)" />
      <circle cx="32" cy="16" r="4" fill="var(--beacon-core)" />
      <path d="M13 46q19 10 38 0M9 55q10-5 20 0t24 0" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>}
  </span>;
}
