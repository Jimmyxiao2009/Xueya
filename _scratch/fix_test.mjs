import fs from "node:fs";
const p = "tests/learning_loop.rs";
let s = fs.readFileSync(p, "utf8");
s = s.replace(`    assert!(outcome.adaptation.is_none());
    assert!(outcome.mastery > 0.5);`, `    assert!(outcome.adaptation.is_none());
    // One right answer nudges mastery, it does not max it out.
    assert!(outcome.mastery > 0.2 && outcome.mastery < 0.6);`);
fs.writeFileSync(p, s);
console.log("ok");
