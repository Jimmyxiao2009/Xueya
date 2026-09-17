# Migration: prototype → application

This documents how `reference/Orbit_motion_continuous_swipe.html` became the
running app, what was kept verbatim, and the rules for changing it later.

**The prototype is not a mock-up that was thrown away.** It is the design source
of truth, it is still in the repository, and every visual decision in the app is
traceable back to a line in it.

---

## 1. Where the prototype lives

| Path | What |
| --- | --- |
| `reference/Orbit_motion_continuous_swipe.html` | the original single-file prototype, unmodified |
| `_scratch/` | throwaway patch scripts used during the port (deleted; never committed) |

The prototype file was **copied**, not moved — the original is still where the
user kept it. 139,381 bytes / 760 lines: CSS on lines 1–234, JS on 234–760.

---

## 2. What was preserved exactly

### CSS

`src/styles/components.css` (55 kB) is the prototype's stylesheet, split into
three files and otherwise untouched:

| Prototype | Now | Contents |
| --- | --- | --- |
| lines 1–234 (layout, components) | `src/styles/components.css` | 575 rules: shell, roadmap, lesson, quiz, plan, dialogs |
| effects inside the same block | `src/styles/effects.css` | scroll panes, acrylic, progressive blur (`.header-blur`, `.soft-blur`), the swipe preview stage, landing states |
| `@keyframes` + reduced-motion blocks | `src/styles/motion.css` | 66 keyframe sets and their `prefers-reduced-motion` neutralisers |
| `:root` custom properties | `src/styles/tokens.css` | every design token, unchanged values |

Mechanical edits only:

* `:root` was removed from `components.css` and lives in `tokens.css` (so the
  token block is one file, imported into `base`).
* `#app` → `#root` (3 occurrences). React mounts into `#root`, and the app frame
  moved from a `<div id="app">` to `#root` + an inner `#screen`.
* Hard-coded six-node geometry on `.road`, `.path` and `.endnote` is kept as the
  default but overridden inline from `src/lib/roadPath.ts`, so routes with a
  different node count lay out correctly. Six nodes still produce the exact
  prototype values (`800 / 923 / 845`), asserted in `src/lib/design.test.ts`.

No rule was renamed, reordered or "tidied".

### JavaScript → TypeScript

The prototype's JS was **not** rewritten as one React component tree. It was
split by responsibility, and the parts that must not re-render React were kept
imperative:

| Prototype | Now | Notes |
| --- | --- | --- |
| `detailDrag`, `swipeTiming`, the pointer/touch/wheel handlers, `settleBack`, `slideAcross` | `src/lib/swipeEngine.ts` | framework-free; drag frames write `transform` directly, so a pointer move never renders React |
| `openLesson` / `returnToRoadmap` clip maths | `src/lib/lessonTransition.ts` | `lessonCardClip` is the prototype's inset arithmetic, character for character |
| `renderHeader`, `softBlur`, `icon()` | `components/layout/AppHeader.tsx`, `components/ui/SoftBlur.tsx`, `lib/icons.ts` | `iconPaths` holds the prototype's SVG path data verbatim |
| the six-node `road` markup | `components/roadmap/RoadmapScreen.tsx` + `lib/roadPath.ts` | the zig-zag is generated instead of hard-coded |
| lesson markup (`renderLesson`) | `components/lesson/LessonScreen.tsx` + `LessonPane.tsx` | 待学 / 资料 panes, the geometry plate SVG, the floating dock |
| `startLearning`, `submit`, `advance`, the chat | `components/learning/*` + `stores/learningStore.ts` | plan, quiz card, feedback, tutor composer |
| `openLayer` / `closeLayer` / blind slats | `components/roadmap/RoadmapLayer.tsx` + `ConfirmDialog.tsx` | native `<dialog>` + `showModal()` instead of a div overlay |
| `animateView`, `render()` | `src/App.tsx` | view switching, underlay/opening orchestration |
| ambient node float | `src/hooks/useAmbientRoadmap.ts` | the same sine mix, one rAF per visible node |
| press/release feedback | `src/hooks/useControlFeedback.ts` | the global `pointerdown` scale + delayed click |
| `toast()` | `stores/uiStore.ts` + `components/layout/Toast.tsx` | writes into `#toast`, same 3s lifetime |

### Numbers that were pinned

These are now asserted by tests so a future refactor cannot quietly soften the
motion design:

| Constant | Value | Test |
| --- | --- | --- |
| lead distance | 18px | `design.test.ts` |
| detent band | `0.31·w → 0.43·w` | `design.test.ts` |
| resistance slopes | `1 / 0.82 / 0.4 / 0.7` | `design.test.ts` |
| travel cap | `0.94·w` | `design.test.ts` |
| default settle | 720ms | `design.test.ts` |
| release curve | `clamp(560, 1000, 820 + d·0.24 − v·140)` | `design.test.ts` |
| commit distance | `min(132, 0.335·frameWidth)` | `swipeEngine.ts` |
| clip radius | 24px, +6px vertical / +8px horizontal | `design.test.ts` |
| node spacing | 140px, first centre 68px, columns 68 / 332 | `design.test.ts` |
| road heights for 6 nodes | 800 / 923 / 845 | `design.test.ts` |

---

## 3. What was added (and why)

The prototype was a front-end demo: its data lived in JS arrays and reset on
reload. Making it an app required a real backend, and the additions were chosen
to *serve* the prototype's loop rather than replace it.

| Added | Why it was needed |
| --- | --- |
| SQLite schema + `db/repo/*` | the prototype's state was in-memory; a route has to survive a restart |
| `curriculum/` deterministic planner | a new topic must produce a real route with no API key |
| `ai/` trait + `MockProvider` | BYOK, with a working offline default |
| `learning/` + `domain/learner.rs` | mastery and confidence, so "adapt" is a real computation |
| `domain/adaptation.rs` + `adaptation_events` | the roadmap must explain *why* it changed |
| `jobs/` | route generation takes seconds; the UI needs real progress |
| `ingest/` + `knowledge/` | the Research layer — sources actually feed the plan |
| `secrets/` | API keys must never reach the WebView |

Nothing in this list changes what the user sees; each one fills in a part of the
loop that the prototype drew but could not execute.

---

## 4. Behaviour that is intentionally identical

These were the highest-risk parts of the port, and each has a test:

* **Swipe** — finger-following drag with the detent band, a commit threshold, a
  fast-flick shortcut, and a spring-back that never loses the current route.
* **Lesson transition** — the tapped node's rect becomes an `inset()` clip on
  the incoming screen; the reverse plays the same timeline backwards.
* **Scroll memory** — returning from a lesson puts the roadmap back at the exact
  offset it had, and a route swipe restores the neighbour's own offset.
* **Quiz semantics** — wrong objective answers stay on the question and shake
  the card; `short` / `image` are submitted, not graded, and never block
  advancing.
* **Adaptation notice** — a wrong answer inserts a purple review node right
  after the failed one, and the home screen explains it.
* **First launch** — both demo routes (射影几何, 线性代数) are present, each with
  its first node already completed and its second marked current.

---

## 5. Deliberate divergences

Small, and each one is a fix rather than a taste change:

1. **`<dialog>` instead of a div overlay.** The prototype hand-rolled the
   roadmap layer's focus handling. A native `<dialog>` + `showModal()` gives the
   top layer, focus trapping and `Esc` for free, with the same visuals.
2. **Generated road geometry.** The prototype hard-coded six nodes. The app
   derives the same curve from the node count so a 3-node or 8-node route is
   still a road.
3. **`#screen` inside `#root`.** Lesson transitions need a sibling element for
   the frozen underlay / outgoing layer. `#root` is the device frame; `#screen`
   is the live view. Absolute children behave identically, and no
   `#root >` direct-child selector existed in the ported CSS.
4. **WAAPI feature detection.** If `Element.animate` is missing (or the OS asks
   for reduced motion) the swipe and the lesson transition resolve instantly
   instead of throwing. The end state is identical.
5. **Option texts instead of indices.** The prototype compared option indices
   while the UI submitted option text; the backend now resolves either, so the
   answer payload is unambiguous.

---

## 6. Rules for future changes

1. **Open the prototype first.** If the change touches anything the prototype
   defines, match it.
2. **Never delete `reference/`.** It is the only complete statement of the
   design.
3. **Port, don't paraphrase.** When a rule has to move, copy it and adjust the
   minimum. Do not re-express it in Tailwind and delete the original.
4. **Pin new motion constants with a test.** A constant that only lives in a
   `.ts` file will drift.
5. **A screen that "works but feels different" is broken.** Check the visual
   regression list at the end of `docs/DESIGN_SYSTEM.md`.
