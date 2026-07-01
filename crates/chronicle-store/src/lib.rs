use chronicle_core::{CanonicalEvent, ProjectRecord, Span};
use rusqlite::{params, Connection, Result as SqlResult};
use std::path::Path;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let mut store = Self { conn };
        store.run_migrations()?;
        Ok(store)
    }

    pub fn open_in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let mut store = Self { conn };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&mut self) -> SqlResult<()> {
        self.conn
            .execute_batch(include_str!("../migrations/001_initial.sql"))?;
        self.conn
            .execute_batch(include_str!("../migrations/002_fts_triggers.sql"))?;
        Ok(())
    }

    pub fn insert_event(&self, event: &CanonicalEvent) -> SqlResult<()> {
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO events (id, timestamp, source, category, type, project, workspace, duration_ms, metadata, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?;
        let now = chrono::Utc::now().timestamp_millis();
        stmt.execute(params![
            event.id.to_string(),
            event.timestamp,
            event.source,
            serde_json::to_string(&event.category).unwrap_or_default(),
            event.r#type,
            event.project,
            event.workspace,
            event.duration_ms,
            event.metadata.to_string(),
            now,
        ])?;
        Ok(())
    }

    pub fn insert_event_batch(&self, events: &[CanonicalEvent]) -> SqlResult<()> {
        let tx = self.conn.unchecked_transaction()?;
        for event in events {
            self.insert_event(event)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn query_events(
        &self,
        since: i64,
        until: Option<i64>,
        limit: u32,
    ) -> SqlResult<Vec<CanonicalEvent>> {
        let until = until.unwrap_or(i64::MAX);
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, timestamp, source, category, type, project, workspace, duration_ms, metadata
             FROM events WHERE timestamp >= ?1 AND timestamp <= ?2
             ORDER BY timestamp DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![since, until, limit], |row| {
            let metadata_str: String = row.get(8)?;
            Ok(CanonicalEvent {
                id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                timestamp: row.get(1)?,
                source: row.get(2)?,
                category: serde_json::from_str(&row.get::<_, String>(3)?)
                    .unwrap_or(chronicle_core::EventCategory::Os),
                r#type: row.get(4)?,
                project: row.get(5)?,
                workspace: row.get(6)?,
                duration_ms: row.get(7)?,
                metadata: serde_json::from_str(&metadata_str).unwrap_or_default(),
                version: "1.0".into(),
            })
        })?;
        rows.collect()
    }

    /// High-signal events for the live activity timeline (excludes noisy filesystem/git/shell).
    pub fn query_activity_events(
        &self,
        since: i64,
        until: Option<i64>,
        limit: u32,
    ) -> SqlResult<Vec<CanonicalEvent>> {
        let until = until.unwrap_or(i64::MAX);
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, timestamp, source, category, type, project, workspace, duration_ms, metadata
             FROM events WHERE timestamp >= ?1 AND timestamp <= ?2
             AND (
               category IN ('\"os\"', '\"shell\"', '\"git\"')
               OR type IN ('file.created', 'file.deleted')
             )
             AND type NOT IN ('git.other', 'file.modified')
             AND metadata NOT LIKE '%\"app_name\":\"chronicle-ui\"%'
             AND metadata NOT LIKE '%\"app_name\":\"Chronicle\"%'
             ORDER BY timestamp DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![since, until, limit], |row| {
            let metadata_str: String = row.get(8)?;
            Ok(CanonicalEvent {
                id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                timestamp: row.get(1)?,
                source: row.get(2)?,
                category: serde_json::from_str(&row.get::<_, String>(3)?)
                    .unwrap_or(chronicle_core::EventCategory::Os),
                r#type: row.get(4)?,
                project: row.get(5)?,
                workspace: row.get(6)?,
                duration_ms: row.get(7)?,
                metadata: serde_json::from_str(&metadata_str).unwrap_or_default(),
                version: "1.0".into(),
            })
        })?;
        rows.collect()
    }

    pub fn insert_span(&self, span: &Span) -> SqlResult<()> {
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO spans (id, trace_id, parent_id, span_type, project, started_at, ended_at, duration_ms, event_count, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?;
        stmt.execute(params![
            span.id.to_string(),
            span.trace_id.to_string(),
            span.parent_id.map(|u| u.to_string()),
            serde_json::to_string(&span.span_type).unwrap_or_default(),
            span.project,
            span.started_at,
            span.ended_at,
            span.duration_ms,
            span.event_count,
            span.metadata.to_string(),
        ])?;
        Ok(())
    }

    pub fn query_spans(&self, since: i64, until: Option<i64>, limit: u32) -> SqlResult<Vec<Span>> {
        let until = until.unwrap_or(i64::MAX);
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, trace_id, parent_id, span_type, project, started_at, ended_at, duration_ms, event_count, metadata
             FROM spans WHERE started_at >= ?1 AND started_at <= ?2
             ORDER BY started_at DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![since, until, limit], |row| {
            let parent_id: Option<String> = row.get(2)?;
            Ok(Span {
                id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                trace_id: row.get::<_, String>(1)?.parse().unwrap_or_default(),
                parent_id: parent_id.and_then(|s| s.parse().ok()),
                span_type: serde_json::from_str(&row.get::<_, String>(3)?)
                    .unwrap_or(chronicle_core::SpanType::Idle),
                project: row.get(4)?,
                started_at: row.get(5)?,
                ended_at: row.get(6)?,
                duration_ms: row.get(7)?,
                event_count: row.get(8)?,
                metadata: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
            })
        })?;
        rows.collect()
    }

    pub fn search_events(&self, query: &str, limit: u32) -> SqlResult<Vec<CanonicalEvent>> {
        let fts_query = build_fts_query(query);
        if fts_query.is_empty() {
            return Ok(Vec::new());
        }
        let mut stmt = self.conn.prepare_cached(
            "SELECT e.id, e.timestamp, e.source, e.category, e.type, e.project, e.workspace, e.duration_ms, e.metadata
             FROM events e JOIN events_fts fts ON e.rowid = fts.rowid
             WHERE events_fts MATCH ?1
             ORDER BY rank LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![fts_query, limit], |row| {
            Ok(CanonicalEvent {
                id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                timestamp: row.get(1)?,
                source: row.get(2)?,
                category: serde_json::from_str(&row.get::<_, String>(3)?)
                    .unwrap_or(chronicle_core::EventCategory::Os),
                r#type: row.get(4)?,
                project: row.get(5)?,
                workspace: row.get(6)?,
                duration_ms: row.get(7)?,
                metadata: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
                version: "1.0".into(),
            })
        })?;
        rows.collect()
    }

    pub fn count_projects(&self) -> SqlResult<u64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
    }

    pub fn query_projects(&self, limit: u32) -> SqlResult<Vec<ProjectRecord>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT name, path, last_active, language FROM projects
             ORDER BY last_active DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(ProjectRecord {
                name: row.get(0)?,
                path: row.get(1)?,
                last_active: row.get(2)?,
                language: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn query_project_by_name(&self, name: &str) -> SqlResult<Option<ProjectRecord>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT name, path, last_active, language FROM projects WHERE name = ?1",
        )?;
        let mut rows = stmt.query_map(params![name], |row| {
            Ok(ProjectRecord {
                name: row.get(0)?,
                path: row.get(1)?,
                last_active: row.get(2)?,
                language: row.get(3)?,
            })
        })?;
        rows.next().transpose()
    }

    pub fn query_spans_for_project(
        &self,
        project: &str,
        since: i64,
        limit: u32,
    ) -> SqlResult<Vec<Span>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, trace_id, parent_id, span_type, project, started_at, ended_at, duration_ms, event_count, metadata
             FROM spans WHERE project = ?1 AND started_at >= ?2
             ORDER BY started_at DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![project, since, limit], |row| {
            let parent_id: Option<String> = row.get(2)?;
            Ok(Span {
                id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                trace_id: row.get::<_, String>(1)?.parse().unwrap_or_default(),
                parent_id: parent_id.and_then(|s| s.parse().ok()),
                span_type: serde_json::from_str(&row.get::<_, String>(3)?)
                    .unwrap_or(chronicle_core::SpanType::Idle),
                project: row.get(4)?,
                started_at: row.get(5)?,
                ended_at: row.get(6)?,
                duration_ms: row.get(7)?,
                event_count: row.get(8)?,
                metadata: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
            })
        })?;
        rows.collect()
    }

    pub fn query_activity_events_for_project(
        &self,
        project: &str,
        since: i64,
        until: Option<i64>,
        limit: u32,
    ) -> SqlResult<Vec<CanonicalEvent>> {
        let until = until.unwrap_or(i64::MAX);
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, timestamp, source, category, type, project, workspace, duration_ms, metadata
             FROM events WHERE project = ?1 AND timestamp >= ?2 AND timestamp <= ?3
             AND (
               category IN ('\"os\"', '\"shell\"', '\"git\"')
               OR type IN ('file.created', 'file.deleted')
             )
             AND type NOT IN ('git.other', 'file.modified')
             ORDER BY timestamp DESC LIMIT ?4",
        )?;
        let rows = stmt.query_map(params![project, since, until, limit], |row| {
            let metadata_str: String = row.get(8)?;
            Ok(CanonicalEvent {
                id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                timestamp: row.get(1)?,
                source: row.get(2)?,
                category: serde_json::from_str(&row.get::<_, String>(3)?)
                    .unwrap_or(chronicle_core::EventCategory::Os),
                r#type: row.get(4)?,
                project: row.get(5)?,
                workspace: row.get(6)?,
                duration_ms: row.get(7)?,
                metadata: serde_json::from_str(&metadata_str).unwrap_or_default(),
                version: "1.0".into(),
            })
        })?;
        rows.collect()
    }

    pub fn query_span_by_id(&self, id: &str) -> SqlResult<Option<Span>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, trace_id, parent_id, span_type, project, started_at, ended_at, duration_ms, event_count, metadata
             FROM spans WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            let parent_id: Option<String> = row.get(2)?;
            Ok(Span {
                id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                trace_id: row.get::<_, String>(1)?.parse().unwrap_or_default(),
                parent_id: parent_id.and_then(|s| s.parse().ok()),
                span_type: serde_json::from_str(&row.get::<_, String>(3)?)
                    .unwrap_or(chronicle_core::SpanType::Idle),
                project: row.get(4)?,
                started_at: row.get(5)?,
                ended_at: row.get(6)?,
                duration_ms: row.get(7)?,
                event_count: row.get(8)?,
                metadata: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
            })
        })?;
        rows.next().transpose()
    }

    pub fn count_events(&self) -> SqlResult<u64> {
        let count: u64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn count_spans(&self) -> SqlResult<u64> {
        let count: u64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM spans", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn upsert_project(&self, name: &str, path: &str, language: Option<&str>) -> SqlResult<()> {
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO projects (name, path, last_active, language)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(name) DO UPDATE SET last_active = ?3, language = COALESCE(?4, language)",
        )?;
        let now = chrono::Utc::now().timestamp_millis();
        stmt.execute(params![name, path, now, language])?;
        Ok(())
    }

    pub fn prune_non_repo_projects(&self) -> SqlResult<usize> {
        let mut stmt = self.conn.prepare("SELECT name, path FROM projects")?;
        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<_, _>>()?;
        drop(stmt);

        let mut removed = 0usize;
        for (name, path) in rows {
            if !is_repo_path(std::path::Path::new(&path)) {
                self.conn
                    .execute("DELETE FROM projects WHERE name = ?1", params![name])?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

fn is_repo_path(path: &std::path::Path) -> bool {
    path.is_absolute() && (path.join(".git").exists() || path.join("Cargo.toml").exists())
}

fn build_fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronicle_core::{CanonicalEvent, EventCategory, Span, SpanType};

    fn setup_store() -> Store {
        let store = Store::open_in_memory().expect("failed to open in-memory store");
        store
    }

    #[test]
    fn test_migration_runs() {
        let store = setup_store();
        assert!(store.count_events().is_ok());
    }

    #[test]
    fn test_insert_and_query_event() {
        let store = setup_store();
        let event = CanonicalEvent::new("test", EventCategory::Git, "commit.created")
            .with_project("chronicle");
        store.insert_event(&event).unwrap();
        let events = store.query_events(0, None, 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].source, "test");
        assert_eq!(events[0].r#type, "commit.created");
    }

    #[test]
    fn test_batch_insert() {
        let store = setup_store();
        let events: Vec<_> = (0..5)
            .map(|_| CanonicalEvent::new("test", EventCategory::Os, "process.focus"))
            .collect();
        store.insert_event_batch(&events).unwrap();
        assert_eq!(store.count_events().unwrap(), 5);
    }

    #[test]
    fn test_query_events_time_range() {
        let store = setup_store();
        let event = CanonicalEvent::new("test", EventCategory::Shell, "command.executed");
        store.insert_event(&event).unwrap();

        let recent = store.query_events(0, None, 10).unwrap();
        assert_eq!(recent.len(), 1);

        let future = store.query_events(i64::MAX - 10000, None, 10).unwrap();
        assert_eq!(future.len(), 0);
    }

    #[test]
    fn test_fallback_to_rowid() {
        // Search instead of FTS5 for empty store
        let store = setup_store();
        let event = CanonicalEvent::new("vscode", EventCategory::Ide, "file.edited");
        store.insert_event(&event).unwrap();

        // FTS5 requires trigger-based sync; this tests the store doesn't crash
        let result = store.search_events("vscode", 10);
        assert!(result.is_ok());
    }

    #[test]
    fn test_insert_and_query_span() {
        let store = setup_store();
        let mut span = Span::new(SpanType::Coding, Some("chronicle".into()));
        span.close();
        store.insert_span(&span).unwrap();

        let spans = store.query_spans(0, None, 10).unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].span_type, SpanType::Coding);
        assert!(spans[0].ended_at.is_some());
    }

    #[test]
    fn test_prune_non_repo_projects() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_project("ghostty", "ghostty", None).unwrap();
        store
            .upsert_project("chronicle", "/tmp/chronicle", None)
            .unwrap();
        std::fs::create_dir_all("/tmp/chronicle/.git").unwrap();

        let removed = store.prune_non_repo_projects().unwrap();
        assert_eq!(removed, 1);

        let projects = store.query_projects(10).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "chronicle");

        let _ = std::fs::remove_dir_all("/tmp/chronicle");
    }

    #[test]
    fn test_upsert_project() {
        let store = setup_store();
        store
            .upsert_project("chronicle", "/dev/chronicle", Some("Rust"))
            .unwrap();
        store
            .upsert_project("chronicle", "/dev/chronicle", Some("Rust"))
            .unwrap(); // upsert
    }

    #[test]
    fn test_empty_store() {
        let store = setup_store();
        assert_eq!(store.count_events().unwrap(), 0);
        assert_eq!(store.count_spans().unwrap(), 0);
        let events = store.query_events(0, None, 10).unwrap();
        assert!(events.is_empty());
    }
}
