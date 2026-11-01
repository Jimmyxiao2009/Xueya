import { Icon } from "./Icon";
import { LighthouseBeacon, type BeaconState } from "./Lighthouse";
import { useThemeStore } from "@/stores/themeStore";
import type { ComponentProps } from "react";

/** Both existing node designs share the same learning state and click target. */
export function RoadmapNodeIcon({ state, icon, variant }: {
  state: BeaconState;
  icon: ComponentProps<typeof Icon>["name"];
  variant: string;
}) {
  const motif = useThemeStore((s) => s.iconMotif);
  if (motif === "lighthouse") return <span className={`node tower-node ${variant}`}><LighthouseBeacon state={state} /></span>;
  return <span className="node-buoy"><span className={`node ${variant}`}>
    <Icon name={icon} />
    {state === "current" ? <span className="node-orbit" /> : null}
  </span></span>;
}
