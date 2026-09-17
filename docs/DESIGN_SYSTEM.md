# Orbit Design System

Orbit's visual language is not a theme that was invented for the app — it was
designed first, in `reference/Orbit_motion_continuous_swipe.html`, and the app
implements it. **The prototype is the design source of truth.** If a screen here
disagrees with the prototype, the screen is wrong.

This document is the contract: tokens, class names, motion constants and the
rules for when to use Tailwind versus the ported design layer.

---

## 1. The one rule

> Orbit is a quiet, warm, paper-like surface where **motion carries meaning**.
> Every animation in the prototype exists to explain a change of state. Never
> trade one away to simplify the code.

Concretely, that rules out:

* Material / Fluent / shadcn components, or any generic "app kit" look.
* A dashboard, sidebar or card grid. Orbit is one continuous surface with a
  single scroll and a single focused action at a time.
* Turning the roadmap into a list. The winding path, the alternating nodes and
  the ambient float **are** the roadmap.
* A ChatGPT-style chat. The learning screen is a lesson with a plan, not a
  message log with a composer at the bottom of an empty page.

---

## 2. Tokens

Two files hold the same literals, on purpose:

| File | Consumed by |
| --- | --- |
| `src/styles/tokens.css` (`:root`) | the ported design layer (`components.css`, `effects.css`, `motion.css`) |
| `src/styles/index.css` (`@theme`) | Tailwind utilities |

**When you change a colour, change it in both.** `tokens.css` names it
`--bg`; `@theme` names it `--color-canvas`; both are `#f8f7f4`.

### Palette

| Meaning | `tokens.css` | `@theme` | Tailwind | Value |
| --- | --- | --- | --- | --- |
| Page background | `--bg` | `--color-canvas` | `bg-canvas` | `#f8f7f4` |
| Raised paper | `--panel` | `--color-paper` | `bg-paper` | `#fffefa` |
| Primary text | `--text` | `--color-primary` | `text-primary` | `#302e2a` |
| Secondary text | `--muted` | `--color-muted` | `text-muted` | `#7e7a72` |
| Hairline | `--line` | `--color-line` | `border-subtle` | `#e8e4dc` |
| Accent (the "continue" colour) | `--accent` | `--color-accent` | `text-accent` `bg-accent` | `#b65f41` |
| Success / mastered | `--success` | `--color-success` | `text-success` | `#587454` |
| Inserted review node | `--purple` | `--color-review` | `text-review` | `#78638e` |
| Quiz node | `--orange` | `--color-quiz` | `text-quiz` | `#a67635` |
| Backdrop behind the device | `--canvas-deep` | `--color-canvas-deep` | `bg-canvas-deep` | `#eae7e0` |

Warm and desaturated. Nothing is pure black or pure white; the strongest ink is
`#1e1b16`, used only for the floating action button's drop edge.

### Type

| Role | Token | Family |
| --- | --- | --- |
| Body / UI | `--font-sans` | system UI + PingFang SC / Microsoft YaHei |
| Display (route title, lesson title) | `--font-display` | Iowan Old Style → Palatino → Songti SC |
| Numbers (progress, indices) | `--font-numeric` | Georgia |

The serif display face is what makes a route title feel like a book chapter
rather than a heading in an app. Do not replace it with a sans-serif "for
consistency".

### Radii, shadows, stacking

`--r-xs 8` · `--r-sm 12` · `--r-md 14` · `--r-lg 17` · `--r-xl 19` · `--r-2xl 24`
· `--r-pill 999`.

`--shadow-panel` (barely-there lift) · `--shadow-float` (the floating action
button: a hard 5px edge plus a soft halo) · `--shadow-press` · `--shadow-device`
(the 70px glow around the 480px frame on desktop).

Stacking: `--z-sticky 8` < `--z-overlay 10` < `--z-dialog 20` < `--z-toast 40`,
plus two transition-only levels: `.lesson-underlay` (25) and
`.lesson-opening` / `#screen.screen-opening` (26).

---

## 3. Tailwind vs. the ported layer

`src/styles/index.css` declares the layer order explicitly:

```
@layer theme, base, components, utilities;
@import "tailwindcss/theme.css"      layer(theme);
@import "tailwindcss/utilities.css"  layer(utilities);
@import "./tokens.css"      layer(base);
@import "./components.css"  layer(components);
@import "./effects.css"     layer(components);
@import "./motion.css"      layer(components);
```

So **Tailwind utilities always beat the ported design layer**, and the ported
layer beats `base`.

Rules of thumb:

| Write it in | When |
| --- | --- |
| Tailwind utility, with a semantic token | layout, spacing, one-off colour, responsive tweaks on a new element |
| `components.css` | anything the prototype already defined: the roadmap, nodes, lesson card, dock, quiz card, plan |
| `effects.css` | masks, progressive blur, acrylic, the swipe preview stage |
| `motion.css` | `@keyframes` and `prefers-reduced-motion` overrides |

Two hard rules:

1. **Never write `bg-[#f8f7f4]`.** Use `bg-canvas`. A raw hex in a component is a
   design-system bug, not a shortcut.
2. **Never add an unlayered rule to `index.css`.** Unlayered CSS outranks *every*
   layer, so a stray top-level rule silently beats the ported design layer. All
   global rules live inside `@layer base`.

Tailwind's preflight is intentionally **not** imported. Orbit ships its own
reset at the top of `components.css`; pulling preflight in would silently change
heading metrics, list styles and button behaviour across the whole product.

---

## 4. Motion

All keyframes live in `src/styles/motion.css` (73 of them), with a
`prefers-reduced-motion` block that neutralises the decorative ones.

### Constants

| Name | Value | Used for |
| --- | --- | --- |
| `--dur-press` | 85ms | button press |
| `--dur-fast` | 210ms | tab and small element changes |
| `--dur-base` | 340ms | view fade |
| `--dur-slow` | 470ms | the lesson clip opening |
| `--dur-settle` | 720ms | a swipe settling into place |
| `--spring` | `cubic-bezier(.22,1.12,.36,1)` | press releases, icon nudges |
| `--ease-out-soft` | `cubic-bezier(.22,.8,.3,1)` | view changes |
| `--ease-settle` | `cubic-bezier(.14,.68,.16,1)` | swipe commit / return |

Under `prefers-reduced-motion: reduce` every duration token collapses to `0ms`.

### Swipe physics

`src/lib/swipeEngine.ts` holds the numbers, and `src/lib/design.test.ts` pins
them.

`detailDrag(raw, width)` — how far the card moves for a given finger travel:

| Band | Behaviour | Slope |
| --- | --- | --- |
| `0 → 18px` (lead) | 1:1, the card "picks up" immediately | `1` |
| `18px → 0.31·w` | eased follow with resistance | `0.82` |
| `0.31·w → 0.43·w` (detent) | heavily compressed — the card feels like it caught | `0.4` |
| `> 0.43·w` | free again, up to a hard cap of `0.94·w` | `0.7` |

The detent is the whole point: it makes the swipe feel *weighted* instead of
elastic. The cap stops a long drag from dragging the destination off-screen.

`swipeTiming(distance, releaseSpeed)`:

* default (programmatic switch): `720ms`, `cubic-bezier(.14,.68,.16,1)`
* after a release: `clamp(560, 1000, 820 + distance·0.24 − speed·140)` — a fast
  flick finishes sooner, a slow drag is allowed to take its time.

Commit rule: `distance > min(132px, 0.335·frameWidth)` **or** a fast flick
(`|raw| > 26 && |velocity| > 0.58`).

### The lesson transition

`lessonCardClip(bounds, frame)` turns the tapped node's rect into

```
inset(top right bottom left round 24px)
```

with 6px of vertical and 8px of horizontal breathing room, clamped so a node at
the screen edge never produces a negative inset. The incoming screen animates
that clip open over 470ms while the outgoing screen dips 390ms behind it, and
the lesson's own content staggers in 225ms from 82ms (+18ms per element). The
reverse is the exact same timeline with `direction: "reverse"` and reversed
delays.

### Ambient roadmap motion

`src/hooks/useAmbientRoadmap.ts` runs one 36-second linear loop per node. Each
node's phase is `index · 0.81`, its amplitude `0.91 + (index % 3) · 0.055`, and
the label's roll is scaled ×0.3 so text stays readable. The loop is driven by a
single `requestAnimationFrame` per visible node, gated by an
`IntersectionObserver` with an `80px` root margin, and paused wholesale while
the roadmap layer is open.

---

## 5. Interaction feedback

`src/hooks/useControlFeedback.ts` is a global press/release layer:

* `pointerdown` scales the control to `0.91` (`.node-hit`, `.primary`, `.send`)
  or `0.965` (everything else) and records the press time.
* The `click` is **held for 52ms** (36ms for `data-motion="menu"`) so the release
  animation is visible before navigation happens, then re-dispatched.
* `data-motion="own"` opts an element out — used for buttons that run their own
  press animation, like the lesson's floating action button.
* `clicksAreSuppressed()` prevents the click that follows a swipe from firing
  twice.

This is why "clicking a node" feels like pressing a physical button rather than
following a hyperlink. It is not decoration; it is the app's whole tactile
vocabulary.

---

## 6. Class reference

The ported layer is organised around a small set of names. New UI should reuse
these rather than inventing parallel ones.

### Shell

`.app-header` · `.eyebrow` · `.subject-switch` · `.subject-name` · `.map-menu` ·
`.header-blur` · `.compact-top` · `.back` · `.scroll-pane` · `.page` ·
`.bottom-notice`

### Roadmap

`.home-page` · `.road` · `.path` · `.completed-path` · `.stop` · `.node` ·
`.node-buoy` · `.label-buoy` · `.node-orbit` · `.now-card` · `.next-arrow` ·
`.endnote` · `.home-footer` · state variants `.node.done` `.node.current`
`.node.review` `.node.quiz`

### Lesson

`.page-title` · `.subtext` · `.pills` · `.pill` · `.lesson-goals` · `.tabs` ·
`.tab` · `.lesson-content` · `.todos` · `.todo` · `.lesson-brief` ·
`.key-points` · `.source-card` · `.source-actions` · `.empty-note` ·
`.swipe-tip` · `.lesson-dock` · `.primary.floating`

### Learning

`.chat-page` · `.chat-header` · `.chat-body` · `.messages` · `.msg` ·
`.composer-wrap` · `.composer` · `.send` · `.composer-note` ·
`.chat-progress.learning-plan` · `.plan-toggle` · `.plan-steps` · `.plan-step`
(`.is-done` `.is-current` `.is-pending`) · `.plan-marker` · `.plan-task` ·
`.plan-state` · `.completion` · `.plan-checkmark` · `.quiz-card` ·
`.quiz-tag` · `.options` · `.option` · `.upload` · `.feedback` · `.quiz-actions`

### Overlays

`.dialog` · `.dialog.roadmap-layer` · `.progress-card` · `.layer-label` ·
`.layer-map` · `.add-unknown` · `.blind-slat` · `.acrylic` · `.header-blur` ·
`.soft-blur` · `#toast` · `.notice-out` · `.swipe-preview-stage` ·
`.lesson-underlay` · `.lesson-opening`

### Utility states

`.swipe-settled` (suppress re-entrance animations on an incoming surface) ·
`.motion-lite` (drop to cheap animations during a swipe) · `.launching`
(the dock button's confirm state) · `.is-closing` (dialog dismissal)

---

## 7. Layout and viewport

* `#root` is the device frame: `max-width: 480px`, centred, with a
  `--shadow-device` glow. The desktop window defaults to 420×880 so the frame
  *is* the window on first launch, and grows into a centred column if resized.
* `#screen` is the single live view inside the frame:
  `position: relative; display: flex; flex-direction: column; flex: 1; min-height: 0`.
  Absolute children (`.app-header`, `.lesson-dock`, `.bottom-notice`) behave
  exactly as they did when they hung off `#root`.
* Safe areas: `padding-top: env(safe-area-inset-top)` on `#root`.
* Visual viewport: `src/hooks/useVisualViewport.ts` writes `--app-height` and
  `--keyboard-inset` from `window.visualViewport`, so the composer lifts above
  the on-screen keyboard instead of being covered by it.
* Roadmap geometry is generated, not hard-coded: `src/lib/roadPath.ts` derives
  `.path` height, `.road` height, `.endnote` top and each `.stop` top from the
  node count. Six nodes reproduce the prototype's `800 / 923 / 845 / 14 + i·140`
  exactly (asserted in `src/lib/design.test.ts`).

---

## 8. Accessibility

* Every icon is `aria-hidden`; its meaning is carried by adjacent text.
* The route switcher is a real `<button aria-haspopup="dialog">`, and the
  roadmap layer is a native `<dialog>` opened with `showModal()` — so focus
  trapping, `Esc` and the top layer come from the platform.
* The plan toggle is `aria-expanded` + `aria-controls`.
* Focus rings use `outline: 2px solid var(--accent)` with a 4px offset; they are
  never removed.
* `prefers-reduced-motion: reduce` is honoured in three places: the duration
  tokens, `useReducedMotion()` in React (which skips the lesson transition and
  the swipe animations entirely), and the `@media` blocks in `motion.css`.

---

## 9. Visual regression checklist

Before claiming a change is done, check these by hand:

1. First launch shows the 射影几何 roadmap with one node completed and one
   "current", the path drawn through both columns, and the endnote below the
   last node.
2. Dragging the home surface left/right: the neighbouring route follows the
   finger, catches in the detent band, and either commits or springs back.
3. Tapping a node: the node card unfolds into the full lesson, the roadmap dips
   behind it, and the lesson's content staggers in.
4. Back from the lesson: the exact reverse, ending with the roadmap at the same
   scroll offset it had before.
5. Swiping between 待学 / 资料 inside a lesson slides the two panes across.
6. Wrong objective answer → the quiz card shakes, the plan does not advance,
   and returning home shows the "为你插入复习" notice and a purple node.
7. The floating action button presses down, scales, and only then navigates.
8. Resizing the desktop window keeps everything inside a centred 480px column.
9. With the OS set to reduce motion, every one of the above still works, just
   instantly.
