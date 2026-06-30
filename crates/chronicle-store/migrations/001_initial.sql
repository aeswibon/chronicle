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

CREATE VIRTUAL TABLE IF NOT EXISTS events_fts USING fts5(
    source, type, project, workspace, metadata,
    content='events',
    content_rowid='rowid'
);

CREATE TABLE IF NOT EXISTS spans (
    id           TEXT PRIMARY KEY,
    trace_id     TEXT NOT NULL,
    parent_id    TEXT,
    span_type    TEXT NOT NULL,
    project      TEXT,
    started_at   INTEGER NOT NULL,
    ended_at     INTEGER,
    duration_ms  INTEGER,
    event_count  INTEGER DEFAULT 0,
    metadata     TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_spans_trace ON spans(trace_id);
CREATE INDEX IF NOT EXISTS idx_spans_time  ON spans(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_spans_proj  ON spans(project);

CREATE TABLE IF NOT EXISTS sessions (
    id          TEXT PRIMARY KEY,
    session_type TEXT NOT NULL DEFAULT 'focus',
    started_at  INTEGER NOT NULL,
    ended_at    INTEGER,
    duration_ms INTEGER,
    project     TEXT,
    span_count  INTEGER DEFAULT 0,
    event_count INTEGER DEFAULT 0,
    summary     TEXT
);

CREATE INDEX IF NOT EXISTS idx_sessions_time ON sessions(started_at DESC);

CREATE TABLE IF NOT EXISTS projects (
    name        TEXT PRIMARY KEY,
    path        TEXT NOT NULL,
    last_active INTEGER NOT NULL,
    language    TEXT,
    repo_url    TEXT
);

CREATE TABLE IF NOT EXISTS plugins (
    name    TEXT PRIMARY KEY,
    version TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    config  TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS schema_version (
    version    INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL
);

INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (1, (unixepoch('now') * 1000));
