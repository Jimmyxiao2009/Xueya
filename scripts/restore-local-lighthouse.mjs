// Recover the actual local Lighthouse scene, not the earlier merged palette.
import fs from 'node:fs';
import postcss from 'postcss';
const source = 'B:/学涯-backup-20260919/src/';
const read = path => fs.readFileSync(source + path, 'utf8');
const write = (path, text) => fs.writeFileSync('src/' + path, text);
write('components/roadmap/LighthouseRoadmapScreen.tsx', read('components/roadmap/RoadmapScreen.tsx').replace('export function RoadmapScreen(', 'export function LighthouseRoadmapScreen('));
write('components/ui/Lighthouse.tsx', read('components/ui/Lighthouse.tsx'));
write('components/ui/Atmosphere.tsx', read('components/ui/Atmosphere.tsx'));
// Adds standalone path legs; existing Buoy geometry and completedPath are identical.
write('lib/roadPath.ts', read('lib/roadPath.ts'));
// Only the shared scene primitives and homepage section. Keep the rewritten OOBE.
const css = postcss.parse(read('styles/lighthouse.css').split('/* First Run:')[0]);
css.walkRules(rule => {
  if (rule.parent.type === 'atrule' && /keyframes$/.test(rule.parent.name)) return;
  rule.selectors = rule.selectors.map(selector => ':root[data-motif="lighthouse"][data-theme] ' + selector);
});
write('styles/lighthouse-scene.css', '/* Recovered from B:/学涯-backup-20260919/src/styles/lighthouse.css.\n * Original homepage/primitives only; scoped to the Lighthouse motif. */\n' + css.toString());
