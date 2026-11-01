import type { CSSProperties } from "react";
import lighthouseAsset from "@/assets/lighthouse.png";

export type BeaconState = "unlit" | "current" | "lit" | "review";

/** The same brand asset, with its lantern isolated so an unlit tower stays dark. */
export function LighthouseBeacon({
  state = "unlit",
  primary = false,
  height,
}: {
  state?: BeaconState;
  primary?: boolean;
  height?: number;
}) {
  return (
    <span
      className={`beacon beacon--${state}${primary ? " beacon--primary" : ""}`}
      style={{ "--beacon-image": `url(${lighthouseAsset})`, ...(height ? { "--tower-height": `${height}px` } : {}) } as CSSProperties}
      aria-hidden="true"
    >
      <span className="beacon-halo" />
      <img className="beacon-tower" src={lighthouseAsset} alt="" draggable={false} />
      <span className="beacon-lantern" />
      <span className="beacon-ground" />
    </span>
  );
}

/**
 * The 学涯 lighthouse: the brand anchor and the visual identity of the AI.
 *
 * The asset is a single lit tower, pre-cropped from the source PNG with its
 * white background made transparent so it sits cleanly in the dark fog. It is
 * never re-generated and never replaced by a generic AI icon. The warm lamp at
 * the top is the product's "light as accent" made literal.
 */
export function Lighthouse({
  height = 26,
  className,
  alt = "灯塔",
}: {
  /** Rendered height in px; width follows the tower's natural aspect ratio. */
  height?: number;
  className?: string;
  alt?: string;
}) {
  return (
    <img
      src={lighthouseAsset}
      alt={alt}
      draggable={false}
      className={className ? `lighthouse ${className}` : "lighthouse"}
      style={{ height }}
    />
  );
}
