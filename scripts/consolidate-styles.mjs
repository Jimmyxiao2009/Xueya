// One-time migration of the existing cascade; keeps property precedence per selector.
import fs from 'node:fs';
import postcss from 'postcss';

const path = 'src/styles/';
const root = postcss.parse(fs.readFileSync(path + 'components.css', 'utf8'));
root.walkRules(r => { if (r.selector === ':root') r.remove(); });
const palettes = {
  '#f8f7f4': '--world-bg', '#fffefa': '--paper-raised', '#302e2a': '--text-primary', '#7e7a72': '--text-muted',
  '#e8e4dc': '--paper-border', '#b65f41': '--beacon-core', '#78638e': '--mastery-review', '#a67635': '--warning', '#587454': '--success',
};
function tokenColor(color, prop) {
  let raw = color.slice(1);
  if (raw.length === 3 || raw.length === 4) raw = [...raw].map(c => c+c).join('');
  const rgb = [0,2,4].map(i => parseInt(raw.slice(i,i+2),16));
  const hi=Math.max(...rgb), lo=Math.min(...rgb), light=(hi+lo)/510;
  let token=palettes['#'+raw.slice(0,6).toLowerCase()];
  if (!token) {
    if (/shadow/.test(prop)) token = '--shadow-ink';
    else if (light > .8) token = /border|stroke/.test(prop) ? '--paper-border' : '--paper-raised';
    else if (light < .27) token = /background/.test(prop) ? '--button-bg' : '--text-primary';
    else if (rgb[1] > rgb[0]*1.04 && rgb[1] > rgb[2]) token = '--success';
    else if (rgb[2] > rgb[0]*1.04) token = '--mastery-review';
    else if (hi-lo>50) token = '--beacon-core';
    else token = /border|stroke/.test(prop) ? '--navigation-line-muted' : '--text-muted';
  }
  if (raw.length === 8) return `color-mix(in srgb,var(${token}) ${Math.round(parseInt(raw.slice(6),16)/255*100)}%,transparent)`;
  return `var(${token})`;
}
function migrateColors(tree) {
  tree.walkDecls(d => {
    d.value=d.value.replace(/rgba\(248\s*,\s*247\s*,\s*244\s*,\s*([.\d]+)\)/g,(_,a)=>`color-mix(in srgb,var(--fog-near) ${Number(a)*100}%,transparent)`)
      .replace(/#[0-9a-f]{3,8}\b/gi,c=>tokenColor(c,d.prop));
  });
}
migrateColors(root);
// Consolidate repeated selectors only within the same conditional scope.
function consolidate(container) {
  const rules = new Map();
  for (const node of [...container.nodes ?? []]) {
    if(node.type==='comment') { node.remove(); continue; }
    if(node.type==='atrule') { if(node.nodes) consolidate(node); continue; }
    if(node.type!=='rule') continue;
    const previous=rules.get(node.selector);
    if(previous) { node.prepend(previous.nodes.map(d=>d.clone())); previous.remove(); }
    const decls=new Map();
    for(const d of [...node.nodes]) {
      if(d.type!=='decl') continue;
      const key=d.prop;
      const old=decls.get(key);
      if(old) { if(old.important && !d.important) { d.remove(); continue; } old.remove(); }
      decls.set(key,d);
    }
    rules.set(node.selector,node);
  }
}
consolidate(root);
const output=Object.fromEntries(['world','effects','roadmap','lesson','overlays','settings','motion'].map(k=>[k,postcss.root()]));
function category(node) {
  if(node.type==='atrule' && /keyframes/.test(node.name)) return 'motion';
  const s=node.selector ?? '';
  if(/blur|veil|fog|acrylic/.test(s)) return 'effects';
  if(/dialog|layer-|roadmap-layer|toast|notice/.test(s)) return 'overlays';
  if(/provider|settings|cloud|quota|device/.test(s)) return 'settings';
  if(/lesson|learning|quiz|question|answer|plan-|chat|message|composer|feedback|completion|option|formula|diagram|resource/.test(s)) return 'lesson';
  if(/road|node|stop|path|endnote|now-card|home-|progress-card/.test(s)) return 'roadmap';
  return 'world';
}
function distribute(nodes, wrap) {
  for(const node of nodes) {
    if(node.type==='atrule' && node.name==='media') {
      const buckets=Object.fromEntries(Object.keys(output).map(k=>[k,node.clone({nodes:[]})]));
      for(const child of node.nodes) buckets[category(child)].append(child.clone());
      for(const [k,v] of Object.entries(buckets)) if(v.nodes.length) output[k].append(v);
    } else output[category(node)].append(node.clone());
  }
}
distribute(root.nodes);
for(const [k,v] of Object.entries(output)) fs.writeFileSync(path+k+'.css', `/* ${k}: consolidated from the existing product cascade. */\n`+v.toString()+'\n');
const onboard=postcss.parse(fs.readFileSync(path+'onboarding.css','utf8'));
migrateColors(onboard);consolidate(onboard);fs.writeFileSync(path+'onboarding.css',onboard.toString());
fs.writeFileSync(path+'components.css','/* Style ownership moved to world, effects, roadmap, lesson, overlays, settings and motion. */\n');
