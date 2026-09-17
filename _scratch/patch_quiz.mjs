import fs from "node:fs";
const path = "src/services/quiz_service.rs";
let s = fs.readFileSync(path, "utf8");

const old = `        if !grade.correct {
            adaptation = Some(
                loop_repo::insert_adaptation(
                    &self.state.pool,
                    &AdaptationEvent::new(
                        course.id,
                        Some(node.id),
                        AdaptationKind::MisconceptionDetected,
                        if grade.misconceptions.is_empty() {
                            format!("「{}」的检查题未通过", node.title)
                        } else {
                            grade.misconceptions[0].clone()
                        },
                        serde_json::json!({
                            "questionId": question.id,
                            "score": grade.score,
                            "answer": answers,
                        }),
                    ),
                )
                .await
                .map(|_| ())
                .map(|_| ())?;
            );
`;
const neu = `        if !grade.correct {
            let event = AdaptationEvent::new(
                course.id,
                Some(node.id),
                AdaptationKind::MisconceptionDetected,
                if grade.misconceptions.is_empty() {
                    format!("「{}」的检查题未通过", node.title)
                } else {
                    grade.misconceptions[0].clone()
                },
                serde_json::json!({
                    "questionId": question.id,
                    "score": grade.score,
                    "answer": answers,
                }),
            );
            loop_repo::insert_adaptation(&self.state.pool, &event).await?;
            adaptation = Some(event);
`;
if (!s.includes(old)) throw new Error("adaptation anchor missing");
s = s.replace(old, neu);

// drop the unused placeholder struct
s = s.replace(`#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TutorEvaluation {
    feedback: String,
    correctness: f64,
    #[serde(default)]
    misconceptions: Vec<String>,
    #[serde(default)]
    suggested_action: String,
}

`, "");
s = s.replace(`use serde::Deserialize;
`, "");
fs.writeFileSync(path, s);
console.log("ok");
