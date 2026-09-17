import type { CSSProperties } from "react";

const RAYS: [number, number][] = [
  [-15, -12],
  [0, -20],
  [16, -12],
  [18, 10],
  [0, 21],
  [-17, 10],
];

/**
 * The hand-drawn tick that marks a finished step, plus its six rays.
 * Kept as a component so the draw-in animations replay every time a step
 * completes rather than only on first paint.
 */
export function CompletionMark() {
  return (
    <>
      <svg className="plan-checkmark" viewBox="0 0 28 28" aria-hidden="true">
        <circle className="check-disc" cx="14" cy="14" r="11" />
        <circle className="check-ring" cx="14" cy="14" r="10" />
        <path className="check-stroke" d="m8.5 14 3.5 3.5 7.5-8" />
      </svg>
      <span className="check-rays" aria-hidden="true">
        {RAYS.map(([x, y], index) => (
          <i
            key={`${x}:${y}`}
            style={
              {
                "--ray-x": `${x}px`,
                "--ray-y": `${y}px`,
                "--ray-turn": `${index * 60}deg`,
              } as CSSProperties
            }
          />
        ))}
      </span>
    </>
  );
}
