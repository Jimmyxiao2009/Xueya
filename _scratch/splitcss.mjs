import fs from "node:fs";
const html = fs.readFileSync("reference/Orbit_motion_continuous_swipe.html", "utf8");
const style = html.match(/<style>([\s\S]*?)<\/style>/)[1];

// Split top-level rules with brace matching.
const rules = [];
let i = 0, buf = "";
while (i < style.length) {
  const ch = style[i];
  if (ch === "{") {
    // find matching brace
    let depth = 1, j = i + 1;
    while (j < style.length && depth > 0) {
      if (style[j] === "{") depth++;
      else if (style[j] === "}") depth--;
      j++;
    }
    rules.push({ prelude: buf.trim(), body: style.slice(i + 1, j - 1), raw: buf.trim() + "{" + style.slice(i + 1, j - 1) + "}" });
    buf = "";
    i = j;
  } else if (ch === "}") { i++; }
  else { buf += ch; i++; }
}

const motion = [], effects = [], components = [], atrules = [];
for (const r of rules) {
  const p = r.prelude;
  if (p.startsWith("@keyframes")) { motion.push(r.raw); continue; }
  if (p.includes("@media") && p.includes("prefers-reduced-motion")) { motion.push(r.raw); continue; }
  if (/soft-blur|acrylic|header-blur|scroll-pane|lesson-underlay|lesson-opening|swipe-preview-stage|swipe-settled|swipe-landing|motion-lite/.test(p)) { effects.push(r.raw); continue; }
  if (p.startsWith("@media")) { atrules.push(r.raw); continue; }
  components.push(r.raw);
}
const header = (name) => `/* ${name}\n * Ported verbatim from reference/Orbit_motion_continuous_swipe.html.\n * Orbit's motion design is the product's signature: do not "clean this up"\n * without re-checking the visual regression checklist in docs/DESIGN_SYSTEM.md.\n */\n`;
fs.mkdirSync("src/styles", { recursive: true });
fs.writeFileSync("src/styles/motion.css", header("motion.css") + motion.join("\n") + "\n");
fs.writeFileSync("src/styles/effects.css", header("effects.css") + effects.join("\n") + "\n" + atrules.filter(r=>/prefers-reduced-motion/.test(r)).join("\n") + "\n");
fs.writeFileSync("src/styles/components.css", header("components.css") + components.join("\n") + "\n" + atrules.join("\n") + "\n");
console.log("rules", rules.length, "motion", motion.length, "effects", effects.length, "components", components.length, "at", atrules.length);
