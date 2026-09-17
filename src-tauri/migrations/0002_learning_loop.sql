-- Verruckt learning-loop schema.
--
-- 0001 modelled research + curriculum (sources -> chunks -> courses -> nodes).
-- 0002 models the *learning* half of the product:
--   learner_states  : per knowledge-point mastery/confidence/scheduling
--   quiz_questions  : the question bank attached to a course node
--   quiz_attempts   : every submitted answer, graded locally or by the tutor
--   adaptation_events: the audit trail of "the roadmap changed itself"
--   learning_messages: the persisted tutor transcript of a session
--   lesson_steps    : the sequential learning plan shown inside a lesson
--
-- All timestamps are RFC3339 UTC TEXT values.

ALTER TABLE courses ADD COLUMN goal TEXT;
ALTER TABLE courses ADD COLUMN experience_level TEXT;
ALTER TABLE courses ADD COLUMN desired_depth TEXT;
ALTER TABLE courses ADD COLUMN kind TEXT NOT NULL DEFAULT 'custom';
ALTER TABLE courses ADD COLUMN current_node_id TEXT;
ALTER TABLE courses ADD COLUMN seeded INTEGER NOT NULL DEFAULT 0;

ALTER TABLE course_nodes ADD COLUMN kind TEXT NOT NULL DEFAULT 'lesson';
ALTER TABLE course_nodes ADD COLUMN confidence REAL NOT NULL DEFAULT 0.0;
ALTER TABLE course_nodes ADD COLUMN review_at TEXT;
ALTER TABLE course_nodes ADD COLUMN generated_by TEXT NOT NULL DEFAULT 'deterministic';

CREATE TABLE IF NOT EXISTS knowledge_points (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    ordinal INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS node_knowledge_points (
    node_id TEXT NOT NULL REFERENCES course_nodes(id) ON DELETE CASCADE,
    knowledge_point_id TEXT NOT NULL REFERENCES knowledge_points(id) ON DELETE CASCADE,
    PRIMARY KEY (node_id, knowledge_point_id)
);

CREATE TABLE IF NOT EXISTS lesson_steps (
    id TEXT PRIMARY KEY NOT NULL,
    node_id TEXT NOT NULL REFERENCES course_nodes(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    title TEXT NOT NULL,
    detail TEXT NOT NULL DEFAULT '',
    kind TEXT NOT NULL DEFAULT 'explain',
    duration_label TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    UNIQUE (node_id, ordinal)
);

CREATE TABLE IF NOT EXISTS learner_states (
    course_id TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    knowledge_point_id TEXT NOT NULL REFERENCES knowledge_points(id) ON DELETE CASCADE,
    confidence REAL NOT NULL DEFAULT 0.0,
    mastery REAL NOT NULL DEFAULT 0.0,
    mistakes INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_seen TEXT,
    next_review TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (course_id, knowledge_point_id)
);

CREATE TABLE IF NOT EXISTS quiz_questions (
    id TEXT PRIMARY KEY NOT NULL,
    node_id TEXT NOT NULL REFERENCES course_nodes(id) ON DELETE CASCADE,
    knowledge_point_id TEXT REFERENCES knowledge_points(id) ON DELETE SET NULL,
    ordinal INTEGER NOT NULL,
    kind TEXT NOT NULL,
    prompt TEXT NOT NULL,
    options_json TEXT NOT NULL DEFAULT '[]',
    answer_json TEXT NOT NULL DEFAULT '[]',
    explanation TEXT NOT NULL DEFAULT '',
    difficulty REAL NOT NULL DEFAULT 0.5,
    created_at TEXT NOT NULL,
    UNIQUE (node_id, ordinal)
);

CREATE TABLE IF NOT EXISTS quiz_attempts (
    id TEXT PRIMARY KEY NOT NULL,
    question_id TEXT NOT NULL REFERENCES quiz_questions(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL REFERENCES course_nodes(id) ON DELETE CASCADE,
    session_id TEXT REFERENCES learning_sessions(id) ON DELETE SET NULL,
    answer_json TEXT NOT NULL,
    correct INTEGER NOT NULL,
    score REAL NOT NULL,
    feedback TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS adaptation_events (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    node_id TEXT REFERENCES course_nodes(id) ON DELETE SET NULL,
    kind TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    payload_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS learning_messages (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES learning_sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_nodes_course_kind ON course_nodes(course_id, kind);
CREATE INDEX IF NOT EXISTS idx_lesson_steps_node ON lesson_steps(node_id, ordinal);
CREATE INDEX IF NOT EXISTS idx_quiz_questions_node ON quiz_questions(node_id, ordinal);
CREATE INDEX IF NOT EXISTS idx_quiz_attempts_node ON quiz_attempts(node_id);
CREATE INDEX IF NOT EXISTS idx_adaptation_course ON adaptation_events(course_id, created_at);
CREATE INDEX IF NOT EXISTS idx_learning_messages_session ON learning_messages(session_id, created_at);
CREATE INDEX IF NOT EXISTS idx_knowledge_points_course ON knowledge_points(course_id, ordinal);

-- Jobs carry their own result so the UI can poll for progress *and* the
-- artefact the job produced (e.g. the id of a freshly generated route).
ALTER TABLE jobs ADD COLUMN result_json TEXT;
