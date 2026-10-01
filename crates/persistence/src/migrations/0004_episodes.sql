CREATE TABLE capture_episodes (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_key TEXT NOT NULL,
    source_json TEXT NOT NULL,
    continuation_id TEXT,
    state TEXT NOT NULL CHECK(state IN ('active','idle','interrupted','finalized','deleted')),
    revision INTEGER NOT NULL CHECK(revision > 0),
    first_sequence INTEGER NOT NULL CHECK(first_sequence > 0),
    last_sequence INTEGER NOT NULL CHECK(last_sequence >= first_sequence),
    last_observed_at TEXT NOT NULL,
    finalized_at TEXT,
    end_reason TEXT,
    paused INTEGER NOT NULL CHECK(paused IN (0,1)),
    restarted INTEGER NOT NULL CHECK(restarted IN (0,1)),
    late_evidence INTEGER NOT NULL CHECK(late_evidence IN (0,1)),
    fingerprint TEXT NOT NULL,
    CHECK(state NOT IN ('finalized') OR finalized_at IS NOT NULL)
);
CREATE UNIQUE INDEX capture_one_live_episode ON capture_episodes(source_key)
    WHERE state IN ('active','idle','interrupted');
CREATE INDEX capture_episode_project ON capture_episodes(project_id,last_observed_at,id);
CREATE TABLE episode_ingress (
    ingress_id TEXT PRIMARY KEY NOT NULL REFERENCES ingress_events(ingress_id) ON DELETE CASCADE,
    episode_id TEXT NOT NULL REFERENCES capture_episodes(id) ON DELETE CASCADE,
    receive_sequence INTEGER NOT NULL CHECK(receive_sequence > 0),
    UNIQUE(episode_id, receive_sequence)
);
CREATE INDEX episode_ingress_order ON episode_ingress(episode_id,receive_sequence,ingress_id);
