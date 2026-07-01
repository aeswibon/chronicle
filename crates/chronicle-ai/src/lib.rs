//! Rule-based summaries and semantic query expansion (no external LLM required).

use chronicle_core::{CanonicalEvent, Session, SessionType, Span};
use std::collections::HashSet;

/// Expand a user query for FTS when semantic mode is requested.
pub fn expand_search_query(query: &str) -> String {
    let lower = query.to_lowercase();
    let expansions: &[(&[&str], &str)] = &[
        (
            &["debug", "debugging", "lldb", "gdb"],
            "debugging lldb gdb error failed panic stack trace",
        ),
        (
            &["test", "testing", "spec"],
            "test iteration cargo test pytest jest vitest failed passed",
        ),
        (
            &["deploy", "deployment", "release"],
            "deployment kubectl helm terraform docker compose fly deploy",
        ),
        (
            &["commit", "git", "merge"],
            "commit merge push branch checkout rebase git",
        ),
        (
            &["build", "compile"],
            "build cargo build npm run build make cmake",
        ),
        (
            &["error", "fail", "broken"],
            "error failed exit non-zero panic exception",
        ),
    ];

    for (needles, extra) in expansions {
        if needles.iter().any(|n| lower.contains(n)) {
            return format!("{query} {extra}");
        }
    }
    query.to_string()
}

/// Build a human-readable daily work summary from spans and events.
pub fn daily_summary(spans: &[Span], events: &[CanonicalEvent]) -> String {
    let mut projects: HashSet<String> = HashSet::new();
    let mut labels: HashSet<String> = HashSet::new();
    let mut errors = 0u32;

    for span in spans {
        if let Some(p) = &span.project {
            projects.insert(p.clone());
        }
        if let Some(obj) = span.metadata.as_object() {
            if let Some(arr) = obj.get("activity_labels").and_then(|v| v.as_array()) {
                for label in arr {
                    if let Some(s) = label.as_str() {
                        labels.insert(s.to_string());
                    }
                }
            }
        }
    }

    for event in events {
        if let Some(p) = &event.project {
            projects.insert(p.clone());
        }
        if let Some(label) = event.metadata.get("activity_label").and_then(|v| v.as_str()) {
            labels.insert(label.to_string());
        }
        if event.category == chronicle_core::EventCategory::Shell {
            if event
                .metadata
                .get("exit_code")
                .and_then(|v| v.as_str())
                .is_some_and(|c| c != "0")
            {
                errors += 1;
            }
        }
    }

    let mut parts = Vec::new();
    parts.push(format!(
        "{} focus session{}",
        spans.len(),
        if spans.len() == 1 { "" } else { "s" }
    ));
    parts.push(format!(
        "{} activity event{}",
        events.len(),
        if events.len() == 1 { "" } else { "s" }
    ));

    if !projects.is_empty() {
        let mut list: Vec<_> = projects.into_iter().collect();
        list.sort();
        parts.push(format!("Projects: {}", list.join(", ")));
    }
    if !labels.is_empty() {
        let mut list: Vec<_> = labels.into_iter().collect();
        list.sort();
        parts.push(format!("Activities: {}", list.join(", ")));
    }
    if errors > 0 {
        parts.push(format!("{errors} failed command{}", if errors == 1 { "" } else { "s" }));
    }

    parts.join(". ") + "."
}

/// Materialize a rollup `Session` row for persistence.
pub fn build_daily_session(
    since: i64,
    until: i64,
    spans: &[Span],
    events: &[CanonicalEvent],
) -> Session {
    let summary = daily_summary(spans, events);
    let duration_ms = (until - since).max(0) as u64;
    let project = spans
        .iter()
        .filter_map(|s| s.project.clone())
        .max_by_key(|p| {
            spans
                .iter()
                .filter(|s| s.project.as_deref() == Some(p.as_str()))
                .count()
        });

    Session {
        id: uuid::Uuid::new_v4(),
        session_type: SessionType::Focus,
        started_at: since,
        ended_at: Some(until),
        duration_ms: Some(duration_ms),
        project,
        span_count: spans.len() as u32,
        event_count: events.len() as u32,
        summary: Some(summary),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronicle_core::{CanonicalEvent, EventCategory};

    #[test]
    fn expands_debug_query() {
        let q = expand_search_query("debugging session");
        assert!(q.contains("lldb"));
    }

    #[test]
    fn summary_lists_projects() {
        let span = Span::new(chronicle_core::SpanType::Coding, Some("chronicle".into()));
        let events = vec![CanonicalEvent::new("zsh", EventCategory::Shell, "command.completed")];
        let text = daily_summary(&[span], &events);
        assert!(text.contains("chronicle"));
    }
}
