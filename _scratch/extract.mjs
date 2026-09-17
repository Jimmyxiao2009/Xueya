import fs from "node:fs";
const html = fs.readFileSync("reference/Orbit_motion_continuous_swipe.html", "utf8");
const style = html.match(/<style>([\s\S]*?)<\/style>/)[1];
const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
// CSS: one declaration per line
let out = [];
let depth = 0;
let buf = "";
for (const ch of style) {
  if (ch === "{") { out.push(" ".repeat(depth*2) + buf.trim() + " {"); buf=""; depth++; }
  else if (ch === "}") { if (buf.trim()) out.push(" ".repeat(depth*2) + buf.trim()); buf=""; depth--; out.push(" ".repeat(depth*2) + "}"); }
  else if (ch === ";") { out.push(" ".repeat(depth*2) + buf.trim() + ";"); buf=""; }
  else buf += ch;
}
fs.writeFileSync("_scratch/orbit.css", out.join("\n"));
fs.writeFileSync("_scratch/orbit.js", script);
fs.writeFileSync("_scratch/orbit.body.html", html.slice(html.indexOf("</style>") + 8, html.indexOf("<script>")));
console.log("css lines", out.length, "js chars", script.length);
console.log("--- selectors ---");
console.log(out.filter(l => l.trim().endsWith("{")).map(l=>l.trim().slice(0,-1)).join(" | "));
