CREATE TABLE assembly_groups (
    group_key TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    adapter_version TEXT NOT NULL,
    transport TEXT NOT NULL,
    client_surface TEXT NOT NULL,
    external_session_id TEXT NOT NULL,
    session_id TEXT NOT NULL UNIQUE,
    state TEXT NOT NULL CHECK (state IN ('assembled', 'deleted')),
    assembly_version INTEGER NOT NULL CHECK (assembly_version > 0),
    evidence_fingerprint TEXT NOT NULL,
    assembled_at TEXT NOT NULL,
    deleted_at TEXT,
    UNIQUE(project_id, provider, adapter_version, transport, client_surface, external_session_id),
    CHECK (
        (state = 'assembled' AND deleted_at IS NULL)
        OR (state = 'deleted' AND deleted_at IS NOT NULL)
    )
);

CREATE INDEX assembly_groups_project_state
    ON assembly_groups(project_id, state, assembled_at);

CREATE TABLE ingress_assembly_state (
    ingress_id TEXT PRIMARY KEY NOT NULL
        REFERENCES ingress_events(ingress_id) ON DELETE CASCADE,
    group_key TEXT REFERENCES assembly_groups(group_key) ON DELETE CASCADE,
    session_id TEXT,
    state TEXT NOT NULL CHECK (state IN ('assigned', 'ignored')),
    reason TEXT,
    assembly_version INTEGER NOT NULL CHECK (assembly_version > 0),
    updated_at TEXT NOT NULL,
    CHECK (
        (state = 'assigned' AND group_key IS NOT NULL AND session_id IS NOT NULL AND reason IS NULL)
        OR (state = 'ignored' AND group_key IS NULL AND session_id IS NULL AND reason IS NOT NULL)
    )
);

CREATE INDEX ingress_assembly_session
    ON ingress_assembly_state(session_id, ingress_id)
    WHERE state = 'assigned';

CREATE INDEX ingress_assembly_group
    ON ingress_assembly_state(group_key, ingress_id)
    WHERE state = 'assigned';
