-- The full schema: placement, the index with reading and current versions,
-- immutable section versions with their records, the course memory, check
-- answers, the glossary, side chats and the local settings.
--
-- Every syncable record has a UUID, creation and update times and a
-- tombstone (ADR-0008); nothing in them is keyed to one machine. Enumerations
-- are stored as snake_case text.

-- Courses: the placement inputs, the stage and where the candidate is. A
-- course from before placement existed already has its index.
ALTER TABLE courses ADD COLUMN self_rating TEXT
    CHECK (self_rating IN ('new_to_this', 'know_the_basics', 'use_it_regularly', 'expert'));
ALTER TABLE courses ADD COLUMN goal TEXT
    CHECK (goal IN ('overview', 'working_knowledge', 'expert'));
ALTER TABLE courses ADD COLUMN stage TEXT NOT NULL DEFAULT 'ready'
    CHECK (stage IN ('placement', 'ready'));
ALTER TABLE courses ADD COLUMN last_opened_at TEXT;
ALTER TABLE courses ADD COLUMN last_read_node_id TEXT REFERENCES index_nodes (id);

-- Section versions are rebuilt around blocks; a version from the thin loop
-- becomes one prose block and has no record.
CREATE TABLE section_versions_full (
    id TEXT PRIMARY KEY NOT NULL,
    node_id TEXT NOT NULL REFERENCES index_nodes (id),
    style TEXT NOT NULL
        CHECK (style IN ('original', 'simpler', 'more_technical', 'more_examples')),
    -- JSON: the shown blocks, the keyed check questions, citations, sources.
    blocks TEXT NOT NULL,
    checks TEXT NOT NULL,
    citations TEXT NOT NULL,
    sources TEXT NOT NULL,
    written_without_search INTEGER NOT NULL CHECK (written_without_search IN (0, 1)),
    -- JSON: the section record, and whether the model's record was confirmed
    -- or the map entry's prediction stands in for it.
    record TEXT,
    record_status TEXT CHECK (record_status IN ('confirmed', 'predicted')),
    model TEXT,
    -- The prompt and its version, such as `write-section@1`.
    prompt TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    CHECK ((record IS NULL) = (record_status IS NULL))
);

INSERT INTO section_versions_full
    (id, node_id, style, blocks, checks, citations, sources, written_without_search,
     record, record_status, model, prompt, created_at, updated_at, deleted_at)
SELECT id, node_id, 'original',
       json_array(json_object('index', 0, 'content', json_object('type', 'Prose', 'text', content))),
       '[]', '[]', '[]', 0, NULL, NULL, NULL, prompt, created_at, updated_at, deleted_at
FROM section_versions;

DROP TABLE section_versions;
ALTER TABLE section_versions_full RENAME TO section_versions;
CREATE INDEX section_versions_by_node ON section_versions (node_id, created_at);

-- A stored version never changes. The one update allowed is its deletion:
-- setting the tombstone, which may erase the content with it.
CREATE TRIGGER section_versions_are_immutable
BEFORE UPDATE ON section_versions
WHEN NOT (OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL
          AND NEW.id = OLD.id AND NEW.node_id = OLD.node_id
          AND NEW.created_at = OLD.created_at)
BEGIN
    SELECT RAISE(ABORT, 'a section version is immutable; only its tombstone can be set');
END;

-- Index nodes: when a section was first read and which version is current.
ALTER TABLE index_nodes ADD COLUMN read_at TEXT;
ALTER TABLE index_nodes ADD COLUMN current_version_id TEXT REFERENCES section_versions (id);

UPDATE index_nodes
SET current_version_id = (SELECT version.id FROM section_versions version
                          WHERE version.node_id = index_nodes.id
                            AND version.deleted_at IS NULL
                          ORDER BY version.created_at DESC, version.rowid DESC LIMIT 1),
    read_at = (SELECT MIN(version.created_at) FROM section_versions version
               WHERE version.node_id = index_nodes.id AND version.deleted_at IS NULL)
WHERE kind = 'section';

-- The current version is always one of the node's own.
CREATE TRIGGER index_nodes_current_version_is_their_own
BEFORE UPDATE OF current_version_id ON index_nodes
WHEN NEW.current_version_id IS NOT NULL
     AND NOT EXISTS (SELECT 1 FROM section_versions version
                     WHERE version.id = NEW.current_version_id
                       AND version.node_id = NEW.id)
BEGIN
    SELECT RAISE(ABORT, 'the current version belongs to another section');
END;

-- Placement: the ladder, the questions asked with their answers, and the
-- candidate profile.
CREATE TABLE ladder_rungs (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT NOT NULL REFERENCES courses (id),
    rung INTEGER NOT NULL CHECK (rung >= 1),
    concept TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE UNIQUE INDEX ladder_rungs_per_course ON ladder_rungs (course_id, rung)
    WHERE deleted_at IS NULL;

CREATE TABLE placement_questions (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT NOT NULL REFERENCES courses (id),
    -- 1 for the first question asked.
    number INTEGER NOT NULL CHECK (number >= 1),
    -- JSON: the keyed question, and the answer once given.
    question TEXT NOT NULL,
    answer TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX placement_questions_by_course ON placement_questions (course_id, number);

CREATE TABLE profile_rungs (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT NOT NULL REFERENCES courses (id),
    rung INTEGER NOT NULL CHECK (rung >= 1),
    state TEXT NOT NULL CHECK (state IN ('known', 'shaky', 'unknown')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE UNIQUE INDEX profile_rungs_per_course ON profile_rungs (course_id, rung)
    WHERE deleted_at IS NULL;

CREATE TABLE misconceptions (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT NOT NULL REFERENCES courses (id),
    rung INTEGER NOT NULL CHECK (rung >= 1),
    text TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX misconceptions_by_course ON misconceptions (course_id);

-- Check answers: the first answer to each question of a version.
CREATE TABLE check_answers (
    id TEXT PRIMARY KEY NOT NULL,
    version_id TEXT NOT NULL REFERENCES section_versions (id),
    block INTEGER NOT NULL CHECK (block >= 0),
    question INTEGER NOT NULL CHECK (question >= 0),
    choice INTEGER NOT NULL CHECK (choice >= 0),
    correct INTEGER NOT NULL CHECK (correct IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE UNIQUE INDEX check_answers_per_question ON check_answers (version_id, block, question)
    WHERE deleted_at IS NULL;

-- The course map (ADR-0013): concepts, and per index node the concepts it
-- introduces and relies on, in order.
CREATE TABLE concepts (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT NOT NULL REFERENCES courses (id),
    name TEXT NOT NULL,
    rung INTEGER CHECK (rung >= 1),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE UNIQUE INDEX concepts_per_course ON concepts (course_id, name) WHERE deleted_at IS NULL;

CREATE TABLE map_entries (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT NOT NULL REFERENCES courses (id),
    node_id TEXT NOT NULL REFERENCES index_nodes (id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE UNIQUE INDEX map_entries_per_node ON map_entries (node_id) WHERE deleted_at IS NULL;
CREATE INDEX map_entries_by_course ON map_entries (course_id);

CREATE TABLE map_entry_concepts (
    id TEXT PRIMARY KEY NOT NULL,
    entry_id TEXT NOT NULL REFERENCES map_entries (id),
    concept_id TEXT NOT NULL REFERENCES concepts (id),
    role TEXT NOT NULL CHECK (role IN ('introduces', 'relies_on')),
    position INTEGER NOT NULL CHECK (position >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX map_entry_concepts_by_entry ON map_entry_concepts (entry_id, role, position);

-- Conventions: the first fixing holds. A key is unique per course and kind
-- once case and surrounding space are normalised; the store writes the
-- normalised key beside the key as given.
CREATE TABLE conventions (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT NOT NULL REFERENCES courses (id),
    kind TEXT NOT NULL CHECK (kind IN ('notation', 'terminology', 'running_example')),
    key TEXT NOT NULL,
    normalised_key TEXT NOT NULL,
    meaning TEXT NOT NULL,
    -- Where it was first fixed: the version whose record fixed it, or NULL
    -- for the plan made with the index.
    origin_version_id TEXT REFERENCES section_versions (id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE UNIQUE INDEX conventions_per_course_kind_key
    ON conventions (course_id, kind, normalised_key) WHERE deleted_at IS NULL;

-- The glossary: each definition anchored to a version, a block and the
-- quoted text, with the Wikipedia lookup once made (JSON).
CREATE TABLE glossary_entries (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT NOT NULL REFERENCES courses (id),
    version_id TEXT NOT NULL REFERENCES section_versions (id),
    block INTEGER NOT NULL CHECK (block >= 0),
    quote TEXT NOT NULL,
    term TEXT NOT NULL,
    definition TEXT NOT NULL,
    wikipedia TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX glossary_entries_by_course ON glossary_entries (course_id, created_at);
CREATE INDEX glossary_entries_by_anchor ON glossary_entries (version_id, block, quote);

-- Side chats, anchored to a passage or to the course only, and their
-- messages.
CREATE TABLE side_chats (
    id TEXT PRIMARY KEY NOT NULL,
    course_id TEXT NOT NULL REFERENCES courses (id),
    version_id TEXT REFERENCES section_versions (id),
    block INTEGER CHECK (block >= 0),
    quote TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    CHECK ((version_id IS NULL) = (block IS NULL) AND (version_id IS NULL) = (quote IS NULL))
);

CREATE INDEX side_chats_by_course ON side_chats (course_id, created_at);
CREATE INDEX side_chats_by_version ON side_chats (version_id);

CREATE TABLE side_chat_messages (
    id TEXT PRIMARY KEY NOT NULL,
    chat_id TEXT NOT NULL REFERENCES side_chats (id),
    role TEXT NOT NULL CHECK (role IN ('candidate', 'agent')),
    text TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('complete', 'stopped')),
    -- JSON.
    citations TEXT NOT NULL,
    sources TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX side_chat_messages_by_chat ON side_chat_messages (chat_id, created_at);

-- Settings of this installation only (the model picked, the host id). Not a
-- syncable record: no UUID, no timestamps, never synced.
CREATE TABLE local_settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) WITHOUT ROWID;
