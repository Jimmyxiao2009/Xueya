-- Verruckt initial schema. All timestamps are RFC3339 UTC TEXT values.

CREATE TABLE IF NOT EXISTS providers (
    id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    chat_model TEXT NOT NULL,
    embedding_model TEXT NOT NULL,
    api_key_ref TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sources (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    uri TEXT,
    original_filename TEXT,
    content_hash TEXT,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS source_revisions (
    id TEXT PRIMARY KEY NOT NULL,
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    revision_no INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    blob_path TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(source_id, revision_no)
);

CREATE TABLE IF NOT EXISTS documents (
    id TEXT PRIMARY KEY NOT NULL,
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    revision_id TEXT NOT NULL REFERENCES source_revisions(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    text_content TEXT NOT NULL,
    page_count INTEGER NOT NULL DEFAULT 0,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS chunks (
    id TEXT PRIMARY KEY NOT NULL,
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    text TEXT NOT NULL,
    char_start INTEGER NOT NULL,
    char_end INTEGER NOT NULL,
    page_number INTEGER,
    heading TEXT,
    token_estimate INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(document_id, ordinal)
);

CREATE TABLE IF NOT EXISTS embeddings (
    id TEXT PRIMARY KEY NOT NULL,
    chunk_id TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    model TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    vector_blob BLOB NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(chunk_id, provider_id, model)
);

CREATE TABLE IF NOT EXISTS knowledge_objects (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS knowledge_relations (
    id TEXT PRIMARY KEY NOT NULL,
    from_object_id TEXT NOT NULL REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    to_object_id TEXT NOT NULL REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL,
    weight REAL NOT NULL DEFAULT 1.0,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    UNIQUE(from_object_id, to_object_id, relation_type)
);

CREATE TABLE IF NOT EXISTS courses (
    id TEXT PRIMARY KEY NOT NULL,
    topic TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    status TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS course_nodes (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    objective TEXT NOT NULL,
    order_hint INTEGER NOT NULL DEFAULT 0,
    estimated_minutes INTEGER NOT NULL DEFAULT 20,
    status TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    lesson_cache_json TEXT,
    lesson_cache_hash TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS course_node_dependencies (
    course_id TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL REFERENCES course_nodes(id) ON DELETE CASCADE,
    depends_on_node_id TEXT NOT NULL REFERENCES course_nodes(id) ON DELETE CASCADE,
    PRIMARY KEY (node_id, depends_on_node_id)
);

CREATE TABLE IF NOT EXISTS learning_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL REFERENCES course_nodes(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    summary_json TEXT
);

CREATE TABLE IF NOT EXISTS learning_attempts (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES learning_sessions(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL REFERENCES course_nodes(id) ON DELETE CASCADE,
    user_answer TEXT NOT NULL,
    evaluation_json TEXT NOT NULL,
    correctness REAL NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS mastery_states (
    node_id TEXT PRIMARY KEY NOT NULL REFERENCES course_nodes(id) ON DELETE CASCADE,
    mastery REAL NOT NULL DEFAULT 0.0,
    attempts INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notes (
    id TEXT PRIMARY KEY NOT NULL,
    object_id TEXT NOT NULL REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    progress REAL NOT NULL DEFAULT 0.0,
    message TEXT NOT NULL DEFAULT '',
    error_json TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT
);

CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT REFERENCES courses(id) ON DELETE CASCADE,
    node_id TEXT REFERENCES course_nodes(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY NOT NULL,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sources_status ON sources(status);
CREATE INDEX IF NOT EXISTS idx_documents_source ON documents(source_id);
CREATE INDEX IF NOT EXISTS idx_chunks_document ON chunks(document_id);
CREATE INDEX IF NOT EXISTS idx_embeddings_model ON embeddings(provider_id, model);
CREATE INDEX IF NOT EXISTS idx_course_nodes_course ON course_nodes(course_id, order_hint);
CREATE INDEX IF NOT EXISTS idx_dependencies_course ON course_node_dependencies(course_id);
CREATE INDEX IF NOT EXISTS idx_sessions_node ON learning_sessions(node_id);
CREATE INDEX IF NOT EXISTS idx_attempts_session ON learning_attempts(session_id);
CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
