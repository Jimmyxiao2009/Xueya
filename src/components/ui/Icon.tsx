import type { CSSProperties } from "react";
import { iconPaths, type IconName } from "@/lib/icons";

export type { IconName };

/**
 * Renders an Orbit icon exactly like the prototype's `I()` helper:
 * `<svg class="icon" viewBox="0 0 24 24" aria-hidden="true">...</svg>`.
 * The `.icon` rule owns stroke width, caps and `currentColor`.
 */
export function Icon({
  name,
  className,
  style,
}: {
  name: IconName;
  className?: string;
  style?: CSSProperties;
}) {
  return (
    <svg
      className={className ? `icon ${className}` : "icon"}
      viewBox="0 0 24 24"
      aria-hidden="true"
      style={style}
      // Static, developer-authored SVG path data - never user input.
      dangerouslySetInnerHTML={{ __html: iconPaths[name] ?? iconPaths.spark }}
    />
  );
}
