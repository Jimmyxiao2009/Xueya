import fs from "node:fs";
const p = "src/lib.rs";
let s = fs.readFileSync(p, "utf8");
s = s.replace(`    courses::{
        course_create, course_delete, course_get, course_list, course_node_get,
        course_node_prepare, course_regenerate,
    },`, `    courses::{
        course_adaptations, course_create, course_delete, course_get, course_learner_states,
        course_list, course_node_complete, course_node_get, course_node_prepare,
        course_regenerate,
    },`);
s = s.replace(`    providers::{provider_delete, provider_list, provider_test, provider_upsert},`, `    providers::{provider_delete, provider_list, provider_test, provider_upsert},
    quiz::{quiz_list, quiz_submit},`);
s = s.replace(`    sources::{
        source_delete, source_import_bytes, source_import_files, source_import_url, source_list,
        source_reindex,
    },
};`, `    sources::{
        source_delete, source_import_bytes, source_import_files, source_import_url, source_list,
        source_reindex,
    },
    tutor::tutor_send,
};`);
fs.writeFileSync(p, s);
console.log("ok");
