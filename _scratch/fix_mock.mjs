import fs from "node:fs";
const p = "src/ai/mock.rs";
let s = fs.readFileSync(p, "utf8");
s = s.replace(
  `use super::traits::{ChatMessage, ChatProvider, ChatRequest, ChatResponse, EmbeddingProvider};`,
  `use super::traits::{ChatProvider, ChatRequest, ChatResponse, EmbeddingProvider};`
);
s = s.replace(
  `        let content = if system.starts_with("You are an instructional designer") {
            course_plan_json(user)?
        } else if system.starts_with("You are a patient tutor") {
            lesson_json(user)?
        } else if system.starts_with("You are a strict but encouraging evaluator") {
            evaluation_json(user)?
        } else if system.starts_with(TUTOR_SYSTEM_PREFIX) {
            tutor_json(user)?
        } else {`,
  `        let content = if system == COURSE_PLAN_SYSTEM {
            course_plan_json(user)?
        } else if system == LESSON_SYSTEM {
            lesson_json(user)?
        } else if system == EVALUATOR_SYSTEM {
            evaluation_json(user)?
        } else if system == TUTOR_SYSTEM {
            tutor_json(user)?
        } else {`
);
s = s.replace(`const TUTOR_SYSTEM_PREFIX: &str = "You are Orbit";

`, "");
s = s.replace(`mod tests {
    use super::*;
    use crate::ai::traits::ChatMessage;`, `mod tests {
    use super::*;
    use crate::ai::traits::ChatMessage;`);
fs.writeFileSync(p, s);
console.log("ok");
