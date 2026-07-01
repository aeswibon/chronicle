//! AI-enhanced summaries and event metadata enrichment.

pub mod context;
pub mod enrich;
pub mod llm;
pub mod rule_summary;
pub mod summary_filter;

pub use context::DayReportContext;
pub use enrich::enrich_event;

use chronicle_config::AiConfig;
use chronicle_core::{CanonicalEvent, Session, SessionType, Span};
use tracing::{debug, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummarySource {
    Ai,
    Rules,
}

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

/// Generate a daily summary using AI when configured, else rule-based fallback.
pub async fn summarize_day(
    ai: &AiConfig,
    since: i64,
    until: i64,
    spans: &[Span],
    events: &[CanonicalEvent],
) -> (String, SummarySource) {
    let prepared: Vec<CanonicalEvent> = events
        .iter()
        .map(|e| {
            let mut e = e.clone();
            enrich_event(&mut e);
            e
        })
        .collect();
    let ctx = DayReportContext::build(since, until, spans, &prepared);
    if ai.enabled {
        match llm::generate_summary(ai, &ctx).await {
            Ok(text) => {
                debug!("AI daily summary generated ({} chars)", text.len());
                return (text, SummarySource::Ai);
            }
            Err(e) => warn!("AI summary failed, using rules: {e}"),
        }
    }
    (
        rule_summary::daily_summary(since, spans, &prepared),
        SummarySource::Rules,
    )
}

/// Materialize a rollup `Session` row for persistence.
pub fn build_daily_session(
    since: i64,
    until: i64,
    spans: &[Span],
    events: &[CanonicalEvent],
    summary: String,
) -> Session {
    let focus_ms: u64 = spans.iter().map(|s| s.duration_ms.unwrap_or(0)).sum();
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
        // Stamp when the summary was generated so the list sorts newest-first.
        started_at: until,
        ended_at: Some(until),
        duration_ms: if focus_ms > 0 {
            Some(focus_ms)
        } else {
            Some((until - since).max(0) as u64)
        },
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
    fn enrich_then_context() {
        let mut e = CanonicalEvent::new("cargo", EventCategory::Shell, "command.failed");
        e.project = Some("chronicle".into());
        e.metadata = serde_json::json!({"command": "cargo test", "exit_code": "1"});
        enrich_event(&mut e);
        let span = Span::new(chronicle_core::SpanType::Coding, Some("chronicle".into()));
        let ctx = DayReportContext::build(0, 1000, &[span], &[e]);
        assert!(!ctx.highlights.is_empty());
    }
}
