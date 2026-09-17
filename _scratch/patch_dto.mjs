import fs from "node:fs";
const path = "src/ipc/dto.rs";
let s = fs.readFileSync(path, "utf8");
const oldStruct = `pub struct LessonDto {
    pub title: String,
    pub explanation: String,
    pub key_points: Vec<String>,
    pub diagnostic_question: String,
    pub citations: Vec<LessonCitationDto>,
}
`;
const newStruct = `pub struct LessonDto {
    pub title: String,
    pub explanation: String,
    pub key_points: Vec<String>,
    pub diagnostic_question: String,
    pub citations: Vec<LessonCitationDto>,
    pub steps: Vec<LessonStepDto>,
    pub questions: Vec<QuizQuestionDto>,
    pub mastery: f64,
    pub estimated_minutes: i64,
    pub node_kind: String,
    /// \`ai\`, \`mock\` or \`deterministic\`. The UI shows this so a template is
    /// never presented as a model answer.
    pub generated_by: String,
    pub has_evidence: bool,
}
`;
if (!s.includes(oldStruct)) throw new Error("lesson struct anchor missing");
s = s.replace(oldStruct, newStruct);

const anchor = `impl LessonDto {
    pub fn from_lesson(lesson: &Lesson, chunks: &[crate::domain::Chunk]) -> Self {`;
const addition = `impl LessonDto {
    /// Build the full learning-screen payload from a prepared node.
    pub fn from_prepared(
        prepared: &crate::services::course_service::PreparedLesson,
        node: &CourseNode,
    ) -> Self {
        let mut dto = Self::from_lesson(&prepared.lesson, &prepared.citations);
        dto.steps = prepared.steps.iter().map(LessonStepDto::from).collect();
        dto.questions = prepared.questions.iter().map(QuizQuestionDto::from).collect();
        dto.mastery = prepared.mastery;
        dto.estimated_minutes = node.estimated_minutes;
        dto.node_kind = node.kind.clone();
        dto.generated_by = prepared.generated_by.clone();
        dto.has_evidence = !prepared.citations.is_empty();
        dto
    }

    pub fn from_lesson(lesson: &Lesson, chunks: &[crate::domain::Chunk]) -> Self {`;
if (!s.includes(anchor)) throw new Error("lesson impl anchor missing");
s = s.replace(anchor, addition);

const tail = `            diagnostic_question: lesson.diagnostic_question.clone(),
            citations,
        }
    }
}`;
const newTail = `            diagnostic_question: lesson.diagnostic_question.clone(),
            citations,
            steps: Vec::new(),
            questions: Vec::new(),
            mastery: 0.0,
            estimated_minutes: 0,
            node_kind: "lesson".to_owned(),
            generated_by: "deterministic".to_owned(),
            has_evidence: false,
        }
    }
}`;
if (!s.includes(tail)) throw new Error("lesson tail anchor missing");
s = s.replace(tail, newTail);
fs.writeFileSync(path, s);
console.log("ok");
