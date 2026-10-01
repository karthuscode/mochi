CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    schema_version INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    started_at_unix_ms INTEGER NOT NULL,
    ended_at TEXT,
    status TEXT NOT NULL,
    source_json TEXT NOT NULL,
    capture_capabilities_json TEXT NOT NULL,
    capture_completeness_json TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX sessions_project_order ON sessions(project_id, started_at_unix_ms DESC, id DESC);

CREATE TABLE session_turns (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    row_json TEXT NOT NULL,
    UNIQUE(session_id, position),
    UNIQUE(session_id, id)
);

CREATE TABLE session_events (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_id TEXT,
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    position INTEGER NOT NULL CHECK (position >= 0),
    row_json TEXT NOT NULL,
    UNIQUE(session_id, sequence),
    UNIQUE(session_id, position),
    FOREIGN KEY(session_id, turn_id) REFERENCES session_turns(session_id, id)
);

CREATE TABLE tool_executions (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_id TEXT,
    position INTEGER NOT NULL CHECK (position >= 0),
    row_json TEXT NOT NULL,
    UNIQUE(session_id, position),
    UNIQUE(session_id, id),
    FOREIGN KEY(session_id, turn_id) REFERENCES session_turns(session_id, id)
);

CREATE TABLE command_executions (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_id TEXT,
    tool_execution_id TEXT,
    position INTEGER NOT NULL CHECK (position >= 0),
    row_json TEXT NOT NULL,
    UNIQUE(session_id, position),
    FOREIGN KEY(session_id, turn_id) REFERENCES session_turns(session_id, id),
    FOREIGN KEY(session_id, tool_execution_id) REFERENCES tool_executions(session_id, id)
);

CREATE TABLE file_changes (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    row_json TEXT NOT NULL,
    UNIQUE(session_id, position)
);

CREATE TABLE git_snapshots (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('before', 'after', 'unavailable')),
    row_json TEXT NOT NULL,
    PRIMARY KEY(session_id, role)
);
