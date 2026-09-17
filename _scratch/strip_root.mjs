import fs from "node:fs";
let css = fs.readFileSync("src/styles/components.css", "utf8");
const start = css.indexOf(":root{");
const end = css.indexOf("}", start);
if (start < 0) throw new Error("no :root");
css = css.slice(0, start) + css.slice(end + 1);
fs.writeFileSync("src/styles/components.css", css);
console.log("removed :root from components.css");
