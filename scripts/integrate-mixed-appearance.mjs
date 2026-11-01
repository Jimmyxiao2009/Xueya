import fs from 'node:fs';
import postcss from 'postcss';

const path = 'src/styles/lighthouse-scene.css';
const css = postcss.parse(fs.readFileSync(path, 'utf8'));
css.walkRules(rule => {
  if (rule.parent.type === 'atrule' && /keyframes$/.test(rule.parent.name)) return;
  rule.selectors = rule.selectors.map(selector => {
    // Tower primitives are needed on either background, including OOBE.
    if (selector.includes(' .beacon')) {
      selector = selector.replace(':root[data-motif="lighthouse"][data-theme]', ':root:is([data-motif="lighthouse"], [data-icons="lighthouse"])[data-theme]');
    }
    // The original scene must not flatten the rounded Buoy node faces.
    if (/\.node(?:\b|[-.])/.test(selector) && /\.node-hit|\.node\.current|\.node-orbit/.test(selector)) {
      selector = selector.replace('[data-theme]', '[data-theme][data-icons="lighthouse"]');
    }
    return selector;
  });
});
fs.writeFileSync(path, css.toString());

// Visible copy only. Keep persistence, protocol and application identifiers stable.
for (const file of fs.readdirSync('src', {recursive:true}).filter(p => /\.tsx$/.test(p))) {
  const path = 'src/' + file;
  let text = fs.readFileSync(path, 'utf8');
  text = text.replaceAll('Verruckt Cloud', '学涯云').replaceAll('Verruckt AI', '学涯 AI').replaceAll('Verruckt', '学涯').replaceAll('Orbit', '学涯').replaceAll('Cloud Sync', '云同步');
  fs.writeFileSync(path, text);
}
for (const path of ['index.html','preview.html','public/admin.html','src-tauri/tauri.conf.json','src-tauri/tauri.android.conf.json']) {
  fs.writeFileSync(path, fs.readFileSync(path,'utf8').replaceAll('Orbit','学涯').replaceAll('Verruckt','学涯'));
}
for (const path of ['src/lib/cloudErrors.ts','src-tauri/src/error.rs','src-tauri/src/ai/registry.rs']) {
  fs.writeFileSync(path, fs.readFileSync(path,'utf8').replaceAll('Verruckt Cloud','学涯云'));
}
const prompts = 'src-tauri/src/ai/prompts.rs';
fs.writeFileSync(prompts, fs.readFileSync(prompts,'utf8').replaceAll('You are Orbit,','You are 学涯,').replaceAll('continue as Orbit.','continue as 学涯.'));
