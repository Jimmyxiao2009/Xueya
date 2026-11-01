export type FogDensity = "light" | "medium" | "dense";

/**
 * Shared atmospheric primitives. Screen-specific CSS only decides placement;
 * this layer supplies the background depth, fog depth and diffuse ambient light.
 */
export function Atmosphere({ density = "medium" }: { density?: FogDensity }) {
  return (
    <div className="atmosphere" data-density={density} aria-hidden="true">
      <div className="atmosphere-distance" />
      <div className="atmosphere-ambient" />
      <div className="atmosphere-fog atmosphere-fog--far" />
      <div className="atmosphere-fog atmosphere-fog--near" />
      <div className="atmosphere-edge" />
    </div>
  );
}

/** A diffuse focus, used beside an active learning state without a hard beam. */
export function FocusGlow({ intensity = "soft" }: { intensity?: "soft" | "strong" }) {
  return <span className={`focus-glow focus-glow--${intensity}`} aria-hidden="true" />;
}
