-- FTS5 sync triggers for external content table
CREATE TRIGGER IF NOT EXISTS events_ai AFTER INSERT ON events BEGIN
  INSERT INTO events_fts(rowid, source, type, project, workspace, metadata)
  VALUES (new.rowid, new.source, new.type, new.project, new.workspace, new.metadata);
END;

CREATE TRIGGER IF NOT EXISTS events_ad AFTER DELETE ON events BEGIN
  INSERT INTO events_fts(events_fts, rowid, source, type, project, workspace, metadata)
  VALUES ('delete', old.rowid, old.source, old.type, old.project, old.workspace, old.metadata);
END;

CREATE TRIGGER IF NOT EXISTS events_au AFTER UPDATE ON events BEGIN
  INSERT INTO events_fts(events_fts, rowid, source, type, project, workspace, metadata)
  VALUES ('delete', old.rowid, old.source, old.type, old.project, old.workspace, old.metadata);
  INSERT INTO events_fts(rowid, source, type, project, workspace, metadata)
  VALUES (new.rowid, new.source, new.type, new.project, new.workspace, new.metadata);
END;

-- Backfill FTS index for events inserted before triggers existed
INSERT INTO events_fts(rowid, source, type, project, workspace, metadata)
SELECT rowid, source, type, project, workspace, metadata FROM events
WHERE rowid NOT IN (SELECT rowid FROM events_fts);

INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (2, (unixepoch('now') * 1000));
