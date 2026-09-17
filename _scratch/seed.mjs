import fs from "node:fs";
let p = "src/db/repo/courses.rs";
let s = fs.readFileSync(p, "utf8");
s += `
/// Flag a route as shipped demo content rather than something the learner made.
pub async fn mark_seeded(pool: &SqlitePool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("UPDATE courses SET seeded = 1, updated_at = ? WHERE id = ?")
        .bind(crate::util::time::utc_now().to_rfc3339())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
`;
fs.writeFileSync(p, s);

p = "src/bootstrap.rs";
s = fs.readFileSync(p, "utf8");
s += `
/// First launch ships the two routes from the design prototype so the app is
/// never an empty shell.
///
/// Both are created through the *same* code path as a user-created route — the
/// deterministic curriculum template, with no sources and no API key — and then
/// flagged \\\`seeded\\\` so they can be told apart from real learner content.
pub async fn seed_demo_routes(state: &AppState) -> Result<(), AppError> {
    if course_repo::count_courses(&state.pool).await? > 0 {
        return Ok(());
    }
    for topic in ["射影几何", "线性代数"] {
        let service = CourseService::new(state);
        let course = service
            .create_course(RouteRequest::new(topic), None)
            .await?;
        course_repo::mark_seeded(&state.pool, course.id).await?;

        // The prototype opens on a route whose first node is already behind you,
        // so the roadmap shows its completed state on first launch.
        let nodes = course_repo::list_nodes(&state.pool, course.id).await?;
        if let Some(first) = nodes.first() {
            service.complete_node(first.id).await?;
        }
    }
    Ok(())
}
`;
s = s.replace(`use crate::error::AppError;
use crate::state::{AppPaths, AppState};
use tauri::Manager;`, `use crate::db::repo::courses as course_repo;
use crate::error::AppError;
use crate::services::course_service::{CourseService, RouteRequest};
use crate::state::{AppPaths, AppState};
use tauri::Manager;`);
fs.writeFileSync(p, s);

p = "src/lib.rs";
s = fs.readFileSync(p, "utf8");
s = s.replace(`            let state = tauri::async_runtime::block_on(bootstrap::build_state(&handle))?;
            app.manage(state);`, `            let state = tauri::async_runtime::block_on(bootstrap::build_state(&handle))?;
            if let Err(error) = tauri::async_runtime::block_on(bootstrap::seed_demo_routes(&state)) {
                tracing::warn!(%error, "could not seed the demo routes");
            }
            app.manage(state);`);
fs.writeFileSync(p, s);
console.log("ok");
