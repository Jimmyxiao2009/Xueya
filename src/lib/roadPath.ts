/**
 * The dotted path that threads the roadmap nodes together.
 *
 * The prototype hard-coded a six-node zig-zag; the same curve is generated here
 * so routes of any length keep the exact look (nodes sit 140px apart and
 * alternate left / right, centres at x = 68 and x = 332).
 */
const STEP = 140;
const FIRST_CENTER = 68;
const LEFT_X = 68;
const RIGHT_X = 332;
/** Node column + trailing space, so `.endnote` never overlaps the last node. */
const TAIL = 123;
const ENDNOTE_OFFSET = 45;

export interface RoadGeometry {
  /** Total height of `.path` in CSS pixels. */
  pathHeight: number;
  /** Total height of `.road`. */
  roadHeight: number;
  /** `top` of `.endnote`. */
  endnoteTop: number;
  viewBox: string;
  full: string;
  /** Standalone curves allow each connection to reflect its actual endpoints. */
  legs: string[];
  /** The travelled part, drawn in the accent colour. */
  completed: string;
  /** `top` for each `.stop`. */
  stopTop: (index: number) => number;
}

export function roadGeometry(count: number): RoadGeometry {
  const n = Math.max(1, count);
  const centers = Array.from({ length: n }, (_, index) => FIRST_CENTER + index * STEP);
  const xAt = (index: number) => (index % 2 === 0 ? LEFT_X : RIGHT_X);
  const segments: string[] = [];
  const legs: string[] = [];
  for (let index = 0; index < n - 1; index += 1) {
    const y0 = centers[index];
    const y1 = centers[index + 1];
    const x0 = xAt(index);
    const x1 = xAt(index + 1);
    legs.push(`M${x0} ${y0}C${x0} ${y0 + (index === 0 ? 56 : 76)} ${x1} ${y1 - 76} ${x1} ${y1}`);
    if (index === 0) {
      segments.push(`M${x0} ${y0}C${x0} ${y0 + 56} ${x1} ${y1 - 76} ${x1} ${y1}`);
    } else {
      segments.push(`S${x1} ${y1 - 76} ${x1} ${y1}`);
    }
  }
  const pathHeight = n === 1 ? 120 : centers[n - 1] + 32;
  return {
    pathHeight,
    roadHeight: pathHeight + TAIL,
    endnoteTop: pathHeight + ENDNOTE_OFFSET,
    viewBox: `0 0 400 ${pathHeight}`,
    full: segments.join(""),
    legs,
    completed: segments.slice(0, Math.max(0, n - 1)).join(""),
    stopTop: (index) => 14 + index * STEP,
  };
}

export function completedPath(geometry: RoadGeometry, completedCount: number): string {
  const segments = geometry.full.match(/[MS][^MS]*/g) ?? [];
  return segments.slice(0, Math.max(0, completedCount)).join("");
}
