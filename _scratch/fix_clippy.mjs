import fs from "node:fs";
let p = "src/curriculum/mod.rs";
let s = fs.readFileSync(p, "utf8");
s = s.replace(`            prompt: format!("本节点标题是「____」。"),`, `            prompt: "本节点标题是「____」。".to_owned(),`);
fs.writeFileSync(p, s);
console.log("ok");
