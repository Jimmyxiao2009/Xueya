use crate::error::AppError;
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use sqlx::sqlite::Sqlite;
use sqlx::{Column, Row, SqlitePool, Transaction, TypeInfo};
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

/// Payloads carry the local schema version so a newer client can decide what to
/// do instead of silently writing unknown columns into an older database.
pub const SCHEMA_VERSION: i64 = 1;

#[async_trait]
pub trait SyncEntityAdapter: Send + Sync {
    fn entity_type(&self) -> &'static str;
    async fn load(&self, pool: &SqlitePool, entity_id: &str) -> Result<Option<Value>, AppError>;
    async fn load_tx(
        &self,
        conn: &mut sqlx::SqliteConnection,
        entity_id: &str,
    ) -> Result<Option<Value>, AppError>;
    async fn apply(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        entity_id: &str,
        payload: &Value,
    ) -> Result<(), AppError>;
    async fn delete(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        entity_id: &str,
    ) -> Result<(), AppError>;
}

/// A normal entity table with a single `id` primary key.
pub struct TableAdapter {
    entity_type: &'static str,
    table: &'static str,
    id_column: &'static str,
}

impl TableAdapter {
    pub const fn new(entity_type: &'static str, table: &'static str) -> Self {
        Self {
            entity_type,
            table,
            id_column: "id",
        }
    }

    pub const fn new_with_id_column(
        entity_type: &'static str,
        table: &'static str,
        id_column: &'static str,
    ) -> Self {
        Self {
            entity_type,
            table,
            id_column,
        }
    }
}

#[async_trait]
impl SyncEntityAdapter for TableAdapter {
    fn entity_type(&self) -> &'static str {
        self.entity_type
    }

    async fn load(&self, pool: &SqlitePool, entity_id: &str) -> Result<Option<Value>, AppError> {
        let sql = format!("SELECT * FROM {} WHERE {} = ?", self.table, self.id_column);
        let row = bind_id(sqlx::query(&sql), entity_id)
            .fetch_optional(pool)
            .await?;
        Ok(row.as_ref().map(row_to_json))
    }

    async fn load_tx(
        &self,
        conn: &mut sqlx::SqliteConnection,
        entity_id: &str,
    ) -> Result<Option<Value>, AppError> {
        let sql = format!("SELECT * FROM {} WHERE {} = ?", self.table, self.id_column);
        let row = bind_id(sqlx::query(&sql), entity_id)
            .fetch_optional(&mut *conn)
            .await?;
        Ok(row.as_ref().map(row_to_json))
    }

    async fn apply(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        _entity_id: &str,
        payload: &Value,
    ) -> Result<(), AppError> {
        apply_payload(tx, self.table, &[self.id_column], payload).await
    }

    async fn delete(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        entity_id: &str,
    ) -> Result<(), AppError> {
        let sql = format!("DELETE FROM {} WHERE {} = ?", self.table, self.id_column);
        bind_id(sqlx::query(&sql), entity_id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
}

/// A join table whose primary key is two foreign keys. The sync entity id is
/// `"first|second"`, so the adapter can address a row without a synthetic id
/// column on the local schema.
pub struct CompositeAdapter {
    entity_type: &'static str,
    table: &'static str,
    first_key: &'static str,
    second_key: &'static str,
}

impl CompositeAdapter {
    pub const fn new(
        entity_type: &'static str,
        table: &'static str,
        first_key: &'static str,
        second_key: &'static str,
    ) -> Self {
        Self {
            entity_type,
            table,
            first_key,
            second_key,
        }
    }

    fn keys(&self) -> [&'static str; 2] {
        [self.first_key, self.second_key]
    }

    fn split<'a>(&self, entity_id: &'a str) -> Result<(&'a str, &'a str), AppError> {
        entity_id.split_once('|').ok_or_else(|| {
            AppError::Internal(format!(
                "invalid composite sync id for {}: {entity_id}",
                self.entity_type
            ))
        })
    }
}

#[async_trait]
impl SyncEntityAdapter for CompositeAdapter {
    fn entity_type(&self) -> &'static str {
        self.entity_type
    }

    async fn load(&self, pool: &SqlitePool, entity_id: &str) -> Result<Option<Value>, AppError> {
        let (first, second) = self.split(entity_id)?;
        let sql = format!(
            "SELECT * FROM {} WHERE {} = ? AND {} = ?",
            self.table, self.first_key, self.second_key
        );
        let row = bind_id(bind_id(sqlx::query(&sql), first), second)
            .fetch_optional(pool)
            .await?;
        Ok(row.as_ref().map(row_to_json))
    }

    async fn load_tx(
        &self,
        conn: &mut sqlx::SqliteConnection,
        entity_id: &str,
    ) -> Result<Option<Value>, AppError> {
        let (first, second) = self.split(entity_id)?;
        let sql = format!(
            "SELECT * FROM {} WHERE {} = ? AND {} = ?",
            self.table, self.first_key, self.second_key
        );
        let row = bind_id(bind_id(sqlx::query(&sql), first), second)
            .fetch_optional(&mut *conn)
            .await?;
        Ok(row.as_ref().map(row_to_json))
    }

    async fn apply(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        _entity_id: &str,
        payload: &Value,
    ) -> Result<(), AppError> {
        apply_payload(tx, self.table, &self.keys(), payload).await
    }

    async fn delete(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        entity_id: &str,
    ) -> Result<(), AppError> {
        let (first, second) = self.split(entity_id)?;
        let sql = format!(
            "DELETE FROM {} WHERE {} = ? AND {} = ?",
            self.table, self.first_key, self.second_key
        );
        bind_id(bind_id(sqlx::query(&sql), first), second)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
}

pub fn composite_id(first: &str, second: &str) -> String {
    format!("{first}|{second}")
}

pub fn adapters() -> Vec<Arc<dyn SyncEntityAdapter>> {
    vec![
        Arc::new(TableAdapter::new("course", "courses")),
        Arc::new(TableAdapter::new("course_node", "course_nodes")),
        Arc::new(CompositeAdapter::new(
            "course_node_dependency",
            "course_node_dependencies",
            "node_id",
            "depends_on_node_id",
        )),
        Arc::new(TableAdapter::new("knowledge_point", "knowledge_points")),
        Arc::new(CompositeAdapter::new(
            "node_knowledge_point",
            "node_knowledge_points",
            "node_id",
            "knowledge_point_id",
        )),
        Arc::new(TableAdapter::new("lesson_step", "lesson_steps")),
        Arc::new(CompositeAdapter::new(
            "learner_state",
            "learner_states",
            "course_id",
            "knowledge_point_id",
        )),
        Arc::new(TableAdapter::new("quiz_question", "quiz_questions")),
        Arc::new(TableAdapter::new("quiz_attempt", "quiz_attempts")),
        Arc::new(TableAdapter::new("adaptation_event", "adaptation_events")),
        Arc::new(TableAdapter::new("note", "notes")),
        Arc::new(TableAdapter::new("knowledge_object", "knowledge_objects")),
        Arc::new(TableAdapter::new(
            "knowledge_relation",
            "knowledge_relations",
        )),
        Arc::new(TableAdapter::new("source", "sources")),
        Arc::new(TableAdapter::new("source_revision", "source_revisions")),
        // Socratic learning state: the local Core remains authoritative, but
        // these entities must travel so another device can resume the same
        // conversation and see the same concept mastery.
        Arc::new(TableAdapter::new("learning_session", "learning_sessions")),
        Arc::new(TableAdapter::new_with_id_column(
            "mastery_state",
            "mastery_states",
            "node_id",
        )),
        Arc::new(TableAdapter::new("learning_message", "learning_messages")),
        Arc::new(TableAdapter::new("learning_attempt", "learning_attempts")),
    ]
}

pub fn adapter_for(entity_type: &str) -> Option<Arc<dyn SyncEntityAdapter>> {
    adapters()
        .into_iter()
        .find(|adapter| adapter.entity_type() == entity_type)
}

fn row_to_json(row: &sqlx::sqlite::SqliteRow) -> Value {
    let mut object = Map::new();
    for column in row.columns() {
        let name = column.name();
        let type_name = column.type_info().name();
        let value = match type_name {
            "INTEGER" | "INT" | "BIGINT" => row
                .try_get::<Option<i64>, _>(name)
                .ok()
                .flatten()
                .map(|value| json!(value)),
            "REAL" | "FLOAT" | "DOUBLE" => row
                .try_get::<Option<f64>, _>(name)
                .ok()
                .flatten()
                .map(|value| json!(value)),
            "BLOB" => row
                .try_get::<Option<Vec<u8>>, _>(name)
                .ok()
                .flatten()
                .map(|value| Value::String(hex::encode(value))),
            _ => row
                .try_get::<Option<String>, _>(name)
                .ok()
                .flatten()
                .map(Value::String)
                .or_else(|| {
                    // UUID primary/foreign keys are declared TEXT in the
                    // migration but sqlx stores them as BLOBs at runtime.
                    row.try_get::<Option<Vec<u8>>, _>(name)
                        .ok()
                        .flatten()
                        .map(|value| Value::String(hex::encode(value)))
                }),
        };
        object.insert(name.to_owned(), value.unwrap_or(Value::Null));
    }
    Value::Object(object)
}

async fn apply_payload(
    tx: &mut Transaction<'_, Sqlite>,
    table: &'static str,
    key_columns: &[&str],
    payload: &Value,
) -> Result<(), AppError> {
    let mut payload = payload.clone();
    // Sessions are device-local; a remote attempt must not reference a session
    // that was never synced to this device.
    if table == "quiz_attempts" {
        payload["session_id"] = Value::Null;
    }
    let object = payload.as_object().ok_or_else(|| {
        AppError::Internal(format!("sync payload for {table} is not a JSON object"))
    })?;

    if let Some(version) = object.get("schema_version") {
        if version.as_i64() != Some(SCHEMA_VERSION) {
            return Err(AppError::CloudSyncConflict(format!(
                "unsupported sync schema_version for {table}: {version}"
            )));
        }
    }

    let allowed = table_columns(tx, table).await?;
    let mut columns: Vec<&str> = Vec::new();
    let mut values: Vec<(&str, &Value)> = Vec::new();
    for column in &allowed {
        if let Some(value) = object.get(column) {
            columns.push(column);
            values.push((column, value));
        }
    }

    for key in key_columns {
        if !columns.contains(key) {
            return Err(AppError::CloudSyncConflict(format!(
                "sync payload for {table} is missing key column {key}"
            )));
        }
    }

    if columns.is_empty() {
        return Err(AppError::Internal(format!(
            "sync payload for {table} has no known columns"
        )));
    }

    let quoted = columns
        .iter()
        .map(|column| quote_ident(column))
        .collect::<Vec<_>>();
    let placeholders = (0..columns.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let updates = columns
        .iter()
        .filter(|column| !key_columns.contains(column))
        .map(|column| format!("{0} = excluded.{0}", quote_ident(column)))
        .collect::<Vec<_>>()
        .join(", ");
    let conflict = key_columns
        .iter()
        .map(|column| quote_ident(column))
        .collect::<Vec<_>>()
        .join(", ");

    let mut sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT({}) DO ",
        quote_ident(table),
        quoted.join(", "),
        placeholders,
        conflict
    );
    if updates.is_empty() {
        sql.push_str("NOTHING");
    } else {
        sql.push_str(&format!("UPDATE SET {updates}"));
    }

    let mut query = sqlx::query(&sql);
    for (column, value) in values {
        query = bind_json_value(query, column, value);
    }
    query.execute(&mut **tx).await?;
    Ok(())
}

fn bind_json_value<'q>(
    query: sqlx::query::Query<'q, Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    column: &str,
    value: &'q Value,
) -> sqlx::query::Query<'q, Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    match value {
        Value::Null => query.bind(Option::<String>::None),
        Value::Bool(value) => query.bind(if *value { 1_i64 } else { 0_i64 }),
        Value::Number(value) => {
            if let Some(integer) = value.as_i64() {
                query.bind(integer)
            } else if let Some(float) = value.as_f64() {
                query.bind(float)
            } else {
                query.bind(value.to_string())
            }
        }
        Value::String(value) => {
            // sqlx stores `Uuid` columns as SQLite BLOBs. Payloads carry them
            // as hex text, so UUID-looking values must be bound back as BLOBs.
            if is_uuid_column(column) {
                if let Ok(uuid) = Uuid::parse_str(value) {
                    return query.bind(uuid);
                }
            }
            query.bind(value.clone())
        }
        other => query.bind(other.to_string()),
    }
}

/// True for columns that hold a local UUID foreign key. `cloud_file_id` is a
/// cloud-side UUID stored as TEXT on purpose, so it is excluded.
fn is_uuid_column(column: &str) -> bool {
    (column == "id" || column.ends_with("_id")) && !column.starts_with("cloud_")
}

/// Bind a local entity id. Simple UUIDs are stored as BLOBs by sqlx, while
/// composite ids are split before reaching this helper.
fn bind_id<'q>(
    query: sqlx::query::Query<'q, Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    value: &'q str,
) -> sqlx::query::Query<'q, Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    match Uuid::parse_str(value) {
        Ok(uuid) => query.bind(uuid),
        Err(_) => query.bind(value),
    }
}

async fn table_columns(
    tx: &mut Transaction<'_, Sqlite>,
    table: &'static str,
) -> Result<HashSet<String>, AppError> {
    let sql = format!("SELECT name FROM pragma_table_info('{table}')");
    let rows = sqlx::query(&sql).fetch_all(&mut **tx).await?;
    let mut columns = HashSet::new();
    for row in rows {
        let name: String = row.try_get("name")?;
        columns.insert(name);
    }
    Ok(columns)
}

fn quote_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrate;
    use sqlx::sqlite::SqliteConnectOptions;
    use std::str::FromStr;

    async fn test_pool() -> SqlitePool {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("sqlite options")
            .foreign_keys(true);
        let pool = SqlitePool::connect_with(options).await.expect("pool");
        migrate::run(&pool).await.expect("migrate");
        pool
    }

    fn uuid(value: &str) -> Uuid {
        Uuid::parse_str(value).expect("uuid")
    }

    fn hex_id(value: &Uuid) -> String {
        hex::encode(value.as_bytes())
    }

    async fn insert_course(pool: &SqlitePool, id: Uuid, title: &str) {
        sqlx::query(
            "INSERT INTO courses (id, topic, title, description, status, created_at, updated_at) \
             VALUES (?, 'geometry', ?, 'desc', 'ready', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(id)
        .bind(title)
        .execute(pool)
        .await
        .expect("insert course");
    }

    async fn insert_course_node(pool: &SqlitePool, course: Uuid, node: Uuid) {
        sqlx::query(
            "INSERT INTO course_nodes (id, course_id, title, description, objective, order_hint, estimated_minutes, status, created_at, updated_at) \
             VALUES (?, ?, 'Node', '', 'Understand', 0, 20, 'in_progress', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(node)
        .bind(course)
        .execute(pool)
        .await
        .expect("insert course node");
    }


    #[tokio::test]
    async fn local_writes_are_journaled_with_monotonic_revisions() {
        let pool = test_pool().await;
        let id = uuid("00000000-0000-0000-0000-000000000001");
        let entity_id = hex_id(&id);
        insert_course(&pool, id, "Vectors").await;

        let (change_id, operation, base_revision, revision): (String, String, i64, i64) =
            sqlx::query_as(
                "SELECT change_id, operation, base_revision, revision FROM sync_outbox \
                 WHERE entity_type = 'course' AND entity_id = ?",
            )
            .bind(&entity_id)
            .fetch_one(&pool)
            .await
            .expect("outbox row");
        assert_eq!(operation, "upsert");
        assert_eq!(base_revision, 0);
        assert_eq!(revision, 1);

        sqlx::query("UPDATE courses SET title = 'Vectors II' WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .expect("update course");
        let (change_id_after, base_after, revision_after): (String, i64, i64) = sqlx::query_as(
            "SELECT change_id, base_revision, revision FROM sync_outbox \
             WHERE entity_type = 'course' AND entity_id = ?",
        )
        .bind(&entity_id)
        .fetch_one(&pool)
        .await
        .expect("outbox row");
        assert_ne!(change_id, change_id_after);
        // base_revision is the last server revision, not the local revision.
        // The first change has never been acknowledged, so it stays 0.
        assert_eq!(base_after, 0);
        assert_eq!(revision_after, 2);
    }

    #[tokio::test]
    async fn apply_guard_keeps_pulled_writes_out_of_the_outbox() {
        let pool = test_pool().await;
        let id = uuid("00000000-0000-0000-0000-000000000002");
        insert_course(&pool, id, "Vectors").await;
        let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_outbox")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(before, 1);

        let mut tx = pool.begin().await.expect("tx");
        sqlx::query("UPDATE sync_apply_guard SET active = 1 WHERE id = 1")
            .execute(&mut *tx)
            .await
            .expect("guard on");
        sqlx::query("UPDATE courses SET title = 'From server' WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .expect("guarded update");
        sqlx::query("UPDATE sync_apply_guard SET active = 0 WHERE id = 1")
            .execute(&mut *tx)
            .await
            .expect("guard off");
        tx.commit().await.expect("commit");

        let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_outbox")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(after, 1);
    }

    #[tokio::test]
    async fn table_adapter_round_trips_a_course_payload() {
        let source = test_pool().await;
        let target = test_pool().await;
        let id = uuid("00000000-0000-0000-0000-000000000003");
        let entity_id = hex_id(&id);
        insert_course(&source, id, "Round trip").await;
        let adapter = adapter_for("course").expect("adapter");
        let mut payload = adapter
            .load(&source, &entity_id)
            .await
            .expect("load")
            .expect("row");
        payload["schema_version"] = json!(SCHEMA_VERSION);

        let mut tx = target.begin().await.expect("tx");
        adapter
            .apply(&mut tx, &entity_id, &payload)
            .await
            .expect("apply");
        tx.commit().await.expect("commit");
        let title: String = sqlx::query_scalar("SELECT title FROM courses WHERE id = ?")
            .bind(id)
            .fetch_one(&target)
            .await
            .expect("title");
        assert_eq!(title, "Round trip");

        let mut tx = target.begin().await.expect("tx");
        adapter.delete(&mut tx, &entity_id).await.expect("delete");
        tx.commit().await.expect("commit");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM courses WHERE id = ?")
            .bind(id)
            .fetch_one(&target)
            .await
            .expect("count");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn composite_learner_state_uses_a_stable_local_id() {
        let pool = test_pool().await;
        let course = uuid("00000000-0000-0000-0000-000000000004");
        let point = uuid("00000000-0000-0000-0000-000000000005");
        let course_hex = hex_id(&course);
        let point_hex = hex_id(&point);
        insert_course(&pool, course, "Composite").await;
        sqlx::query(
            "INSERT INTO knowledge_points (id, course_id, title, description, ordinal, created_at) \
             VALUES (?, ?, 'Point', '', 0, '2026-01-01T00:00:00Z')",
        )
        .bind(point)
        .bind(course)
        .execute(&pool)
        .await
        .expect("knowledge point");
        sqlx::query(
            "INSERT INTO learner_states (course_id, knowledge_point_id, confidence, mastery, mistakes, attempts, updated_at) \
             VALUES (?, ?, 0.5, 0.4, 1, 2, '2026-01-01T00:00:00Z')",
        )
        .bind(course)
        .bind(point)
        .execute(&pool)
        .await
        .expect("learner state");

        let local_id = composite_id(&course_hex, &point_hex);
        let adapter = adapter_for("learner_state").expect("adapter");
        let payload = adapter
            .load(&pool, &local_id)
            .await
            .expect("load")
            .expect("row");
        assert_eq!(payload["course_id"], course_hex);
        assert_eq!(payload["knowledge_point_id"], point_hex);
        assert_eq!(payload["attempts"], 2);

        let outbox_id: String = sqlx::query_scalar(
            "SELECT entity_id FROM sync_outbox WHERE entity_type = 'learner_state'",
        )
        .fetch_one(&pool)
        .await
        .expect("outbox");
        assert_eq!(outbox_id, local_id);
    }

    #[tokio::test]
    async fn socratic_learning_entities_are_journaled_and_registered() {
        let pool = test_pool().await;
        let course = uuid("00000000-0000-0000-0000-000000000006");
        let node = uuid("00000000-0000-0000-0000-000000000007");
        let session = uuid("00000000-0000-0000-0000-000000000008");
        let message = uuid("00000000-0000-0000-0000-000000000009");
        let attempt = uuid("00000000-0000-0000-0000-00000000000a");
        insert_course(&pool, course, "Socratic").await;
        sqlx::query(
            "INSERT INTO course_nodes (id, course_id, title, description, objective, order_hint, estimated_minutes, status, created_at, updated_at) \
             VALUES (?, ?, 'Node', '', 'Understand', 0, 20, 'in_progress', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(node)
        .bind(course)
        .execute(&pool)
        .await
        .expect("course node");
        sqlx::query(
            "INSERT INTO learning_sessions (id, course_id, node_id, status, started_at, summary_json) \
             VALUES (?, ?, ?, 'active', '2026-01-01T00:00:00Z', '{}')",
        )
        .bind(session)
        .bind(course)
        .bind(node)
        .execute(&pool)
        .await
        .expect("learning session");
        sqlx::query(
            "INSERT INTO learning_messages (id, session_id, role, content, metadata_json, created_at) \
             VALUES (?, ?, 'assistant', 'hello', '{}', '2026-01-01T00:00:00Z')",
        )
        .bind(message)
        .bind(session)
        .execute(&pool)
        .await
        .expect("learning message");
        sqlx::query(
            "INSERT INTO learning_attempts (id, session_id, node_id, user_answer, evaluation_json, correctness, created_at) \
             VALUES (?, ?, ?, 'answer', '{}', 0.5, '2026-01-01T00:00:00Z')",
        )
        .bind(attempt)
        .bind(session)
        .bind(node)
        .execute(&pool)
        .await
        .expect("learning attempt");
        sqlx::query(
            "INSERT INTO mastery_states (node_id, mastery, attempts, updated_at) \
             VALUES (?, 0.5, 1, '2026-01-01T00:00:00Z')",
        )
        .bind(node)
        .execute(&pool)
        .await
        .expect("mastery state");

        let types: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT entity_type FROM sync_outbox ORDER BY entity_type",
        )
        .fetch_all(&pool)
        .await
        .expect("outbox types");
        for expected in [
            "learning_session",
            "learning_message",
            "learning_attempt",
            "mastery_state",
        ] {
            assert!(types.iter().any(|value| value == expected), "missing {expected}");
            assert!(adapter_for(expected).is_some(), "missing adapter {expected}");
        }
    }

    #[tokio::test]
    async fn learning_session_and_conversation_round_trip_between_clients() {
        let source = test_pool().await;
        let target = test_pool().await;
        let course = uuid("00000000-0000-0000-0000-00000000000b");
        let node = uuid("00000000-0000-0000-0000-00000000000c");
        let session = uuid("00000000-0000-0000-0000-00000000000d");
        let message = uuid("00000000-0000-0000-0000-00000000000e");

        insert_course(&source, course, "Cross device").await;
        insert_course_node(&source, course, node).await;
        insert_course(&target, course, "Cross device").await;
        insert_course_node(&target, course, node).await;
        sqlx::query(
            "INSERT INTO learning_sessions (id, course_id, node_id, status, started_at, summary_json) \
             VALUES (?, ?, ?, 'active', '2026-01-01T00:00:00Z', '{}')",
        )
        .bind(session)
        .bind(course)
        .bind(node)
        .execute(&source)
        .await
        .expect("source session");
        sqlx::query(
            "INSERT INTO learning_messages (id, session_id, role, content, metadata_json, created_at) \
             VALUES (?, ?, 'assistant', 'hello from device A', '{}', '2026-01-01T00:00:00Z')",
        )
        .bind(message)
        .bind(session)
        .execute(&source)
        .await
        .expect("source message");
        sqlx::query(
            "INSERT INTO mastery_states (node_id, mastery, attempts, updated_at) \
             VALUES (?, 0.6, 2, '2026-01-01T00:00:00Z')",
        )
        .bind(node)
        .execute(&source)
        .await
        .expect("source mastery");

        for (entity, local_id) in [
            ("learning_session", hex_id(&session)),
            ("learning_message", hex_id(&message)),
            ("mastery_state", hex_id(&node)),
        ] {
            let adapter = adapter_for(entity).expect("adapter");
            let mut payload = adapter
                .load(&source, &local_id)
                .await
                .expect("load")
                .expect("row");
            payload["schema_version"] = json!(SCHEMA_VERSION);
            let mut tx = target.begin().await.expect("tx");
            adapter
                .apply(&mut tx, &local_id, &payload)
                .await
                .expect("apply");
            tx.commit().await.expect("commit");
        }

        let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM learning_sessions")
            .fetch_one(&target)
            .await
            .expect("session count");
        let messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM learning_messages")
            .fetch_one(&target)
            .await
            .expect("message count");
        assert_eq!(sessions, 1);
        assert_eq!(messages, 1);
        let mastery = adapter_for("mastery_state")
            .expect("mastery adapter")
            .load(&target, &hex_id(&node))
            .await
            .expect("load mastery");
        assert!(mastery.is_some());
    }


}
