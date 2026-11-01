/**
 * 学涯's progressive glass edges.
 *
 * Exactly three `<i>` passes: `styles/effects.css` overrides the blur radius and
 * mask stops for `nth-child(1..3)`. Adding or removing a pass changes the look.
 */
export function SoftBlur({ edge = "top" }: { edge?: "top" | "bottom" }) {
  return (
    <div
      className={`soft-blur${edge === "bottom" ? " soft-blur-bottom" : ""}`}
      aria-hidden="true"
    >
      <i />
      <i />
      <i />
    </div>
  );
}

export function HeaderBlur() {
  return (
    <div className="header-blur" aria-hidden="true">
      <i />
      <i />
      <i />
    </div>
  );
}
