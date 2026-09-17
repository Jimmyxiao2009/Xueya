import fs from "node:fs";
const p = "src/lib.rs";
let s = fs.readFileSync(p, "utf8");
s = s.replace(`            course_node_get,
            course_node_prepare,
            learning_start,`, `            course_node_get,
            course_node_prepare,
            course_node_complete,
            course_learner_states,
            course_adaptations,
            quiz_list,
            quiz_submit,
            tutor_send,
            learning_start,`);
fs.writeFileSync(p, s);
console.log("ok");
