CREATE TABLE projects (
    id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    root_path TEXT NOT NULL,
    repository_identity_json TEXT,
    created_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    tracking_enabled INTEGER NOT NULL CHECK (tracking_enabled IN (0, 1)),
    policy_revision INTEGER NOT NULL CHECK (policy_revision > 0),
    deleted_at TEXT
);
CREATE UNIQUE INDEX projects_active_root ON projects(root_path) WHERE deleted_at IS NULL;

CREATE TABLE ingress_events (
    ingress_id TEXT PRIMARY KEY NOT NULL,
    source_identity_key TEXT NOT NULL UNIQUE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    schema_version INTEGER NOT NULL,
    provider TEXT NOT NULL,
    adapter_version TEXT NOT NULL,
    transport TEXT NOT NULL,
    client_surface TEXT NOT NULL,
    source_event_id TEXT,
    source_sequence INTEGER,
    source_timestamp TEXT,
    received_at TEXT NOT NULL,
    received_at_unix_ms INTEGER NOT NULL,
    receive_sequence INTEGER NOT NULL CHECK (receive_sequence > 0),
    source_event_type TEXT NOT NULL,
    external_session_id TEXT,
    external_turn_id TEXT,
    external_tool_use_id TEXT,
    origin TEXT NOT NULL,
    normalized_event_json TEXT NOT NULL,
    sensitivity_json TEXT NOT NULL,
    imported_at TEXT NOT NULL
);
CREATE INDEX ingress_project_order ON ingress_events(project_id, receive_sequence, ingress_id);
CREATE INDEX ingress_external_session ON ingress_events(project_id, external_session_id);
CREATE INDEX ingress_retention ON ingress_events(received_at_unix_ms);

CREATE TABLE ingress_rejections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    token_hash TEXT NOT NULL,
    reason TEXT NOT NULL,
    receive_sequence INTEGER,
    rejected_at TEXT NOT NULL,
    UNIQUE(token_hash, reason)
);

CREATE TABLE ingress_tombstones (
    identity_key TEXT PRIMARY KEY NOT NULL,
    identity_kind TEXT NOT NULL,
    reason TEXT NOT NULL,
    deleted_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    expires_at_unix_ms INTEGER NOT NULL
);
CREATE INDEX ingress_tombstone_expiry ON ingress_tombstones(expires_at_unix_ms);

CREATE TABLE import_state (
    singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
    evicted_count INTEGER NOT NULL,
    last_eviction_at TEXT,
    observed_at TEXT NOT NULL
);
