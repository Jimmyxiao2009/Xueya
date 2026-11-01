/** Recover only visual deltas from the already merged Lighthouse design.
 * Layout and interaction stay in the user's pre-turn components.css.
 */
import fs from 'node:fs';
import { execFileSync } from 'node:child_process';
import postcss from 'postcss';

const original = file => execFileSync('git', ['show', `9dcc5ac:src/styles/${file}`], {encoding:'utf8'});
const baseline = postcss.parse(fs.readFileSync('audit-results/theme-integration/baseline-components.css','utf8'));
const lighthouse = postcss.parse(original('components.css') + '\n' + original('effects.css'));
const visual = /^(color|background(?:-color|-image)?|box-shadow|border(?:-(?:top|right|bottom|left))?(?:-color)?|fill|stroke|text-shadow)$/;
function values(root) {
  const rules = new Map();
  root.walkRules(rule => {
    if(rule.parent.type !== 'root') return;
    for(const selector of postcss.list.comma(rule.selector)) {
      const declarations=rules.get(selector) ?? new Map();
      rule.walkDecls(d => {if(visual.test(d.prop)) declarations.set(d.prop,d.clone());});
      rules.set(selector,declarations);
    }
  });
  return rules;
}
const old = values(baseline), restored=values(lighthouse), output=postcss.root();
const scope=':root[data-motif="lighthouse"][data-theme="dark"]';
for(const [selector, declarations] of restored) {
  if(!old.has(selector) || selector === ':root') continue;
  const prefixed = /^(html|:root)\b/.test(selector) ? selector.replace(/^(html|:root)/,scope) : `${scope} ${selector}`;
  const rule=postcss.rule({selector:prefixed});
  for(const [prop,d] of declarations) if(old.get(selector)?.get(prop)?.value!==d.value) rule.append(d);
  if(rule.nodes.length) output.append(rule);
}
const tokens=postcss.parse(original('tokens.css'));
const colors=postcss.rule({selector:scope});
tokens.nodes.find(n=>n.type==='rule'&&n.selector===':root').walkDecls(d=>{
  if(d.prop.startsWith('--') && !/^--(r-|dur-|z-|content-|keyboard-|spring|ease-)/.test(d.prop)) colors.append(d.clone());
});
output.prepend(colors);
fs.writeFileSync('src/styles/lighthouse-theme.css', '/* Existing Lighthouse / Dark Fog visuals from git HEAD (9dcc5ac).\n * Only palette/surface deltas; original homepage geometry and motion are retained. */\n'+output.toString()+'\n');
