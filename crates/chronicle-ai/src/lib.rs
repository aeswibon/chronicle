//! AI-enhanced summaries and event metadata enrichment.

pub mod context;
pub mod enrich;
pub mod llm;
pub mod rule_summary;
pub mod summary_filter;

pub use context::DayReportContext;
pub use enrich::enrich_event;
pub use llm::{list_ollama_models, test_connection};

use chronicle_config::AiConfig;
use chronicle_core::{CanonicalEvent, Session, SessionType, Span};
use tracing::{debug, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummarySource {
    Ai,
    Rules,
}

#[derive(Debug, Clone)]
pub struct SummarizeOutcome {
    pub summary: String,
    pub source: SummarySource,
    pub ai_error: Option<String>,
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
) -> SummarizeOutcome {
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
                if let Some(summary) = llm::polish_summary(&text) {
                    debug!("AI daily summary generated ({} chars)", summary.len());
                    return SummarizeOutcome {
                        summary,
                        source: SummarySource::Ai,
                        ai_error: None,
                    };
                }
                warn!("AI summary looked like chain-of-thought, using rules");
                return SummarizeOutcome {
                    summary: rule_summary::daily_summary(since, until, spans, &prepared),
                    source: SummarySource::Rules,
                    ai_error: Some(
                        "AI returned reasoning text instead of a summary; used rules-based rollup"
                            .into(),
                    ),
                };
            }
            Err(e) => {
                let msg = e.to_string();
                warn!("AI summary failed, using rules: {msg}");
                return SummarizeOutcome {
                    summary: rule_summary::daily_summary(since, until, spans, &prepared),
                    source: SummarySource::Rules,
                    ai_error: Some(msg),
                };
            }
        }
    }
    SummarizeOutcome {
        summary: rule_summary::daily_summary(since, until, spans, &prepared),
        source: SummarySource::Rules,
        ai_error: None,
    }
}

/// Stable id for a calendar-day rollup so regenerate/replace does not orphan UI rows.
pub fn daily_session_id(since: i64) -> uuid::Uuid {
    let date = chrono::DateTime::from_timestamp_millis(since)
        .map(|dt| dt.with_timezone(&chrono::Local).date_naive().to_string())
        .unwrap_or_else(|| since.to_string());
    uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        format!("chronicle-daily:{date}").as_bytes(),
    )
}

/// Materialize a rollup `Session` row for persistence.
pub fn build_daily_session(
    since: i64,
    until: i64,
    spans: &[Span],
    events: &[CanonicalEvent],
    summary: String,
    summary_source: &str,
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
        id: daily_session_id(since),
        session_type: SessionType::Focus,
        started_at: since,
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
        summary_source: Some(summary_source.to_string()),
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
    fn daily_session_id_is_stable_for_same_day() {
        let since = chrono::Local::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .and_then(|t| t.and_local_timezone(chrono::Local).single())
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(0);
        assert_eq!(daily_session_id(since), daily_session_id(since));
    }

    #[test]
    fn session_id_serializes_as_string() {
        use chronicle_core::{Session, SessionType};
        let session = Session {
            id: daily_session_id(0),
            session_type: SessionType::Focus,
            started_at: 1,
            ended_at: None,
            duration_ms: None,
            project: None,
            span_count: 0,
            event_count: 0,
            summary: Some("hello".into()),
            summary_source: Some("rules".into()),
        };
        let json = serde_json::to_value(&session).unwrap();
        assert!(json.get("id").and_then(|v| v.as_str()).is_some());
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
