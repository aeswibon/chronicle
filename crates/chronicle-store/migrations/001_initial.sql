CREATE TABLE IF NOT EXISTS events (
    id          TEXT PRIMARY KEY,
    timestamp   INTEGER NOT NULL,
    source      TEXT NOT NULL,
    category    TEXT NOT NULL,
    type        TEXT NOT NULL,
    project     TEXT,
    workspace   TEXT,
    duration_ms INTEGER,
    metadata    TEXT NOT NULL DEFAULT '{}',
    created_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_ts ON events(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_events_proj ON events(project);
CREATE INDEX IF NOT EXISTS idx_events_cat ON events(category);

CREATE TABLE IF NOT EXISTS schema_version (
    version    INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL
);
