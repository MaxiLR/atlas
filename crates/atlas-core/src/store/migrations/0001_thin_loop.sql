-- Courses, their index and generated sections. Every record has a UUID,
-- creation and update times, and a tombstone (ADR-0008).

CREATE TABLE courses (
    id TEXT PRIMARY KEY NOT NULL,
    topic TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE TABLE index_nodes (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT NOT NULL REFERENCES courses (id),
    parent_id TEXT REFERENCES index_nodes (id),
    kind TEXT NOT NULL CHECK (kind IN ('chapter', 'section')),
    position INTEGER NOT NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX index_nodes_by_course ON index_nodes (course_id, parent_id, position);

CREATE TABLE section_versions (
    id TEXT PRIMARY KEY NOT NULL,
    node_id TEXT NOT NULL REFERENCES index_nodes (id),
    content TEXT NOT NULL,
    prompt TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX section_versions_by_node ON section_versions (node_id, created_at);
