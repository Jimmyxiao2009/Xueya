from pathlib import Path

def edit(path, old, new):
    p = Path(path)
    s = p.read_text(encoding='utf-8')
    assert old in s, (path, old[:80])
    p.write_text(s.replace(old, new), encoding='utf-8')

r = 'src-tauri/src/ai/registry.rs'
p = Path(r); s = p.read_text(); a = s.index('        match self.ai_mode().await? {', s.index('pub async fn is_mock_active')); b = s.index('\n    pub async fn ai_mode', a)
s = s[:a] + '        let (config, _) = self.default_chat_provider().await?;\n        Ok(is_mock_config(&config))\n    }\n' + s[b:]; p.write_text(s)
edit(r, '        match cloud.cached_entitlements().await {', '''        if !matches!(cloud.account_state().await, Ok(state) if state.state == crate::cloud::models::CloudSessionState::Authenticated) {
            return false;
        }
        match cloud.cached_entitlements().await {''')
edit(r, '        self.ensure_mock_provider().await?;', '''        if std::env::var("ORBIT_REQUIRE_AI").as_deref() == Ok("1") {
            return Err(AppError::InvalidInput("Real AI is required but no usable provider is available".into()));
        }
        self.ensure_mock_provider().await?;''')
edit(r, '/// The offline provider is a real backend provider, not a frontend fake.', '/// Deterministic offline implementation; never a real AI model.')

c = 'src-tauri/src/services/course_service.rs'
p = Path(c); s = p.read_text(); a=s.index('    /// Create a roadmap.'); b=s.index('    pub async fn create_course',a)
s=s[:a]+'    /// Local orchestration: real AI with sources or general knowledge; explicit offline fallback.\n'+s[b:]
a=s.index('        let planned = if '); b=s.index('\n        if let Some(handle)',a)
s=s[:a]+'''        let prompt = format!("主题：{}\\n目标：{}\\n现有基础：{}\\n学习深度：{}", topic,
            request.goal.as_deref().unwrap_or("理解并能应用"),
            request.experience_level.as_deref().unwrap_or("beginner"),
            request.desired_depth.as_deref().unwrap_or("practical"));
        let (planned, origin) = match self.plan_with_ai(&prompt, &evidence, handle).await {
            Ok(plan) => plan,
            Err(error) => {
                if std::env::var("ORBIT_REQUIRE_AI").as_deref() == Ok("1") { return Err(error); }
                tracing::warn!(%error, "AI unavailable; using deterministic roadmap");
                (curriculum::plan_route(&request.brief()), "deterministic".to_owned())
            }
        };
''' + s[b:]
s=s.replace('        let origin = planned.origin.as_str().to_owned();\n','')
s=s.replace('Ok(chunks) if !chunks.is_empty() => return chunks,\n                Ok(_) => {}', 'Ok(chunks) => return chunks,')
s=s.replace('Err(error) => tracing::debug!(%error, "source-scoped retrieval failed"),','Err(error) => {\n                    tracing::debug!(%error, "source-scoped retrieval failed; scope remains closed");\n                    return Vec::new();\n                }')
s=s.replace('        match knowledge.semantic_search(topic, 12).await {', '''        // Empty selection means topic-only. Global knowledge use must be explicit.
        Vec::new()
        /* match knowledge.semantic_search(topic, 12).await {''',1).replace('    async fn plan_with_ai(', '    async fn plan_with_ai(',1)
a=s.index('        /* match knowledge.semantic_search'); b=s.index('\n    async fn plan_with_ai',a); s=s[:a]+'    }\n'+s[b:]
s=s.replace(') -> Result<PlannedRoute, AppError> {', ') -> Result<(PlannedRoute, String), AppError> {',1)
s=s.replace('        let messages = course_plan_messages(topic, &snippets)?;', '''        crate::ai::provenance::require_real(&provider_config)?;
        let origin = crate::ai::provenance::generation_label(&provider_config, !evidence.is_empty());
        let messages = course_plan_messages(topic, &snippets)?;''')
s=s.replace('Ok(planned_route_from_ai(plan))','Ok((planned_route_from_ai(plan), origin))')
s=s.replace('        let pairs: Vec<(Uuid, Option<String>, String)> = evidence', '        crate::ai::provenance::require_real(&config)?;\n        let pairs: Vec<(Uuid, Option<String>, String)> = evidence',1)
s=s.replace('let node = CourseNode {', 'let mut node = CourseNode {',1)
s=s.replace('"origin": origin,\n                })', '"origin": origin,\n                    "lessonOrigin": "deterministic",\n                })',1)
s=s.replace('let ai_quiz = if origin == "ai" {','let ai_quiz = if origin != "deterministic" {')
s=s.replace('            let planned_questions = ai_quiz.unwrap_or_else(|| {', '''            let mut metadata: serde_json::Value = serde_json::from_str(&node.metadata_json).unwrap_or_default();
            metadata["quizOrigin"] = json!(if ai_quiz.is_some() { origin } else { "deterministic" });
            node.metadata_json = metadata.to_string();
            let planned_questions = ai_quiz.unwrap_or_else(|| {''')
s=s.replace('// Try the model only when there is something to ground it in.\n        if !evidence.is_empty() || std::env::var("ORBIT_REQUIRE_AI").as_deref() == Ok("1") {', '// Provider availability, not evidence presence, determines AI use.\n        {')
s=s.replace('                Ok(lesson) => {\n                    return Ok(PreparedLesson', '                Ok((lesson, origin)) => {\n                    return Ok(PreparedLesson',1)
s=s.replace('generated_by: "ai".to_owned(),','generated_by: origin,',1)
s=s.replace('generated_by: node.generated_by.clone(),', '''generated_by: if cached_origin(&node) != "deterministic" && node.lesson_cache_json.is_some() {
                cached_origin(&node)
            } else { "deterministic".to_owned() },''',1)
s=s.replace(') -> Result<Lesson, AppError> {\n        let source_hash', ') -> Result<(Lesson, String), AppError> {\n        let source_hash',1)
s=s.replace('if !cached.citations.is_empty()\n                    || std::env::var("ORBIT_REQUIRE_AI").as_deref() == Ok("1")', 'if cached_origin(node) != "deterministic"')
s=s.replace('return Ok(cached);','return Ok((cached, cached_origin(node)));',1)
s=s.replace('        let messages = crate::ai::prompts::lesson_messages(', '''        crate::ai::provenance::require_real(&provider_config)?;
        let origin = crate::ai::provenance::generation_label(&provider_config, !evidence.is_empty());
        let messages = crate::ai::prompts::lesson_messages(''',1)
s=s.replace('        course_repo::update_node_lesson_cache(\n            &self.state.pool,\n            node.id,\n            &lesson_json,\n            &source_hash,\n        )\n        .await?;\n        Ok(lesson)', '''        let mut metadata: serde_json::Value = serde_json::from_str(&node.metadata_json).unwrap_or_default();
        metadata["lessonOrigin"] = json!(origin);
        sqlx::query("UPDATE course_nodes SET lesson_cache_json = ?, lesson_cache_hash = ?, metadata_json = ? WHERE id = ?")
            .bind(&lesson_json).bind(&source_hash).bind(metadata.to_string()).bind(node.id)
            .execute(&self.state.pool).await?;
        Ok((lesson, origin))''')
s += '''
fn cached_origin(node: &CourseNode) -> String {
    serde_json::from_str::<serde_json::Value>(&node.metadata_json).ok()
        .and_then(|v| v.get("lessonOrigin").and_then(|s| s.as_str()).map(str::to_owned))
        .unwrap_or_else(|| "deterministic".to_owned())
}
'''
p.write_text(s,encoding='utf-8')

h='src-tauri/src/knowledge/hybrid.rs'
edit(h, '            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&course.metadata_json) {', '            let mut explicit_sources = false;\n            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&course.metadata_json) {')
edit(h, '                    for value in values {', '                    explicit_sources = !values.is_empty();\n                    for value in values {')
edit(h, '            for node in course_repo::list_nodes(&self.state.pool, route_id).await? {\n                route_ids.extend(evidence_ids_from_metadata(&node.metadata_json));\n            }', '''            // Explicit source selection is authoritative even if historical node metadata is contaminated.
            if !explicit_sources {
                for node in course_repo::list_nodes(&self.state.pool, route_id).await? {
                    route_ids.extend(evidence_ids_from_metadata(&node.metadata_json));
                }
            }''')

edit('src-tauri/src/ai/mod.rs', 'pub mod prompts;', 'pub mod prompts;\npub mod provenance;')
for path in ['src-tauri/src/services/tutor_service.rs']:
    edit(path, 'generated_by: if is_mock { "mock".into() } else { "ai".into() },', 'generated_by: crate::ai::provenance::generation_label(&config, !evidence_pairs.is_empty()),')
