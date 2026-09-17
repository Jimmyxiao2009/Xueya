/**
 * Verbatim icon paths from the Orbit prototype. The SVG wrapper lives in
 * `components/ui/Icon.tsx`; keeping the raw paths here makes the visual
 * language copy-pasteable and diffable against the reference file.
 */
export const iconPaths = {
  orbit:
    '<ellipse cx="12" cy="12" rx="11" ry="5" transform="rotate(-35 12 12)"/><circle cx="12" cy="12" r="2"/>',
  arrow: '<path d="M5 12h14m-6-6 6 6-6 6"/>',
  back: '<path d="m14 5-7 7 7 7"/>',
  chevron: '<path d="m9 5 7 7-7 7"/>',
  check: '<path d="m5 12 4 4L19 6"/>',
  infinity: '<path d="M12 12c-3-6-10-6-10 0s7 6 10 0 10-6 10 0-7 6-10 0Z"/>',
  line: '<path d="M3 18 21 6M3 6l18 12M12 3v18"/><circle cx="12" cy="12" r="3"/>',
  review:
    '<path d="M3 10a9 9 0 1 1 2 8M3 4v6h6M12 7v5l3 2"/>',
  quiz: '<path d="m12 2 3 6 7 1-5 5 1 7-6-3-6 3 1-7-5-5 7-1Z"/>',
  grid: '<rect x="3" y="3" width="18" height="18" rx="3"/><path d="M9 3v18M15 3v18M3 9h18M3 15h18"/>',
  spark: '<path d="m12 3 2.3 6.7L21 12l-6.7 2.3L12 21l-2.3-6.7L3 12l6.7-2.3L12 3ZM20 2v4M18 4h4"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  book: '<path d="M12 5v16M12 5C8 2 4 3 2 4v15c4-1 7-1 10 2 3-3 6-3 10-2V4c-3-1-6-2-10 1Z"/>',
  play: '<path d="m8 4 12 8-12 8Z"/>',
  camera: '<path d="m8 5 2-3h4l2 3h5v16H3V5Z"/><circle cx="12" cy="12" r="4"/>',
  send: '<path d="M12 19V5m-6 6 6-6 6 6"/>',
  close: '<path d="m6 6 12 12M6 18 18 6"/>',
  trash: '<path d="M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7M14 10v7"/>',
  clock: '<circle cx="12" cy="12" r="9"/><path d="M12 6v6l4 2"/>',
  upload: '<path d="M12 16V3m-5 5 5-5 5 5M3 15v6h18v-6"/>',
  nodes:
    '<circle cx="5" cy="5" r="3"/><circle cx="19" cy="12" r="3"/><circle cx="5" cy="19" r="3"/><path d="m8 6 8 5M8 18l8-5"/>',
} as const;

export type IconName = keyof typeof iconPaths;

export function iconSvg(name: IconName): string {
  return iconPaths[name] ?? iconPaths.spark;
}
