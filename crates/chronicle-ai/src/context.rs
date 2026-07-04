//! Structured digest for LLM daily summaries.

use crate::summary_filter::{
    is_high_signal, is_meaningful_failure, is_summary_noise, rank_projects,
};
use chronicle_core::{CanonicalEvent, Span};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize)]
pub struct DayReportContext {
    pub since: i64,
    pub until: i64,
    pub stats: DayStats,
    pub highlights: Vec<HighlightLine>,
    pub span_work: Vec<SpanWorkDigest>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpanWorkDigest {
    pub started_at: i64,
    pub project: Option<String>,
    pub span_type: String,
    pub commands: Vec<String>,
    pub git: Vec<String>,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayStats {
    pub span_count: usize,
    pub event_count: usize,
    pub project_count: usize,
    pub error_count: u32,
    pub projects: Vec<String>,
    pub intents: Vec<String>,
    pub focus_minutes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HighlightLine {
    pub timestamp: i64,
    pub intent: String,
    pub outcome: String,
    pub project: Option<String>,
    pub line: String,
}

impl DayReportContext {
    pub fn build(since: i64, until: i64, spans: &[Span], events: &[CanonicalEvent]) -> Self {
        let filtered: Vec<CanonicalEvent> = events
            .iter()
            .filter(|e| !is_summary_noise(e))
            .cloned()
            .collect();
        let stats = compute_stats(spans, &filtered);
        let highlights = select_highlights(&filtered);
        let span_work = collect_span_work(spans);
        Self {
            since,
            until,
            stats,
            highlights,
            span_work,
        }
    }

    pub fn to_prompt_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "Period: {} – {} (local time)\n",
            format_local_ts(self.since),
            format_local_ts(self.until)
        ));
        out.push_str(&format!("Date: {}\n", format_local_date(self.since)));
        out.push_str(&format!(
            "Stats: {} spans, {} events, {} projects, {} errors, ~{}m focused\n",
            self.stats.span_count,
            self.stats.event_count,
            self.stats.project_count,
            self.stats.error_count,
            self.stats.focus_minutes
        ));
        if !self.stats.projects.is_empty() {
            out.push_str(&format!("Projects: {}\n", self.stats.projects.join(", ")));
        }
        if !self.stats.intents.is_empty() {
            out.push_str(&format!("Intents: {}\n", self.stats.intents.join(", ")));
        }
        if !self.span_work.is_empty() {
            out.push_str("\nFocus sessions with captured work:\n");
            for s in &self.span_work {
                let proj = s
                    .project
                    .as_deref()
                    .map(|p| format!(" [{p}]"))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "- {} {}{proj}\n",
                    format_local_ts(s.started_at),
                    s.span_type
                ));
                for c in &s.commands {
                    out.push_str(&format!("  shell: {c}\n"));
                }
                for g in &s.git {
                    out.push_str(&format!("  git: {g}\n"));
                }
                for f in &s.files {
                    out.push_str(&format!("  file: {f}\n"));
                }
            }
        }
        out.push_str("\nHighlights (chronological):\n");
        for h in &self.highlights {
            let proj = h
                .project
                .as_deref()
                .map(|p| format!(" [{p}]"))
                .unwrap_or_default();
            out.push_str(&format!(
                "- {} {}{}{} — {}\n",
                format_local_ts(h.timestamp),
                h.intent,
                if h.outcome == "failure" {
                    " (failed)"
                } else {
                    ""
                },
                proj,
                h.line
            ));
        }
        out
    }
}

fn collect_span_work(spans: &[Span]) -> Vec<SpanWorkDigest> {
    let mut out = Vec::new();
    for span in spans {
        let Some(obj) = span.metadata.as_object() else {
            continue;
        };
        let commands = json_string_list(obj.get("recent_commands"));
        let git = json_string_list(obj.get("recent_git"));
        let files = json_string_list(obj.get("recent_files"));
        if commands.is_empty() && git.is_empty() && files.is_empty() {
            continue;
        }
        out.push(SpanWorkDigest {
            started_at: span.started_at,
            project: span.project.clone(),
            span_type: format!("{:?}", span.span_type),
            commands,
            git,
            files,
        });
    }
    out
}

fn json_string_list(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn compute_stats(spans: &[Span], events: &[CanonicalEvent]) -> DayStats {
    let mut intents: HashSet<String> = HashSet::new();
    let mut error_count = 0u32;
    let mut focus_ms = 0u64;

    let event_refs: Vec<&CanonicalEvent> = events.iter().collect();
    let ranked = rank_projects(spans, &event_refs);
    let project_count = ranked.len();

    for span in spans {
        if span.span_type != chronicle_core::SpanType::Idle {
            focus_ms += span.duration_ms.unwrap_or(0);
        }
    }

    for event in events {
        if let Some(i) = event.metadata.get("intent").and_then(|v| v.as_str()) {
            intents.insert(i.to_string());
        }
        if is_meaningful_failure(event) {
            error_count += 1;
        }
    }

    let project_list: Vec<String> = ranked.into_iter().take(5).map(|(name, _)| name).collect();
    let mut intent_list: Vec<_> = intents.into_iter().collect();
    intent_list.sort();
    let focus_minutes = (focus_ms / 60_000).min(12 * 60);

    DayStats {
        span_count: spans.len(),
        event_count: events.len(),
        project_count,
        error_count,
        projects: project_list,
        intents: intent_list,
        focus_minutes,
    }
}

fn select_highlights(events: &[CanonicalEvent]) -> Vec<HighlightLine> {
    let mut scored: Vec<(i32, &CanonicalEvent)> = events
        .iter()
        .map(|e| (score_event(e), e))
        .filter(|(score, _)| *score > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.timestamp.cmp(&b.1.timestamp)));

    let mut seen: HashMap<String, u32> = HashMap::new();
    let mut out = Vec::new();

    for (_, event) in scored {
        if out.len() >= 48 {
            break;
        }
        let line = event
            .metadata
            .get("report_line")
            .and_then(|v| v.as_str())
            .unwrap_or(&event.r#type)
            .to_string();
        let key = format!("{:?}:{}", event.category, line);
        let count = seen.entry(key).or_insert(0);
        if *count >= 2 {
            continue;
        }
        *count += 1;

        out.push(HighlightLine {
            timestamp: event.timestamp,
            intent: event
                .metadata
                .get("intent")
                .and_then(|v| v.as_str())
                .unwrap_or("work")
                .to_string(),
            outcome: event
                .metadata
                .get("outcome")
                .and_then(|v| v.as_str())
                .unwrap_or("neutral")
                .to_string(),
            project: event.project.clone(),
            line,
        });
    }

    out.sort_by_key(|h| h.timestamp);
    out
}

fn score_event(event: &CanonicalEvent) -> i32 {
    if is_summary_noise(event) {
        return 0;
    }
    let mut score = 1;
    if event.category == chronicle_core::EventCategory::Git {
        if matches!(
            event.r#type.as_str(),
            "fetch.completed" | "pull.completed" | "branch.checkout"
        ) {
            return 0;
        }
        score += 20;
    }
    if event.category == chronicle_core::EventCategory::Ide {
        score += 8;
    }
    if event.category == chronicle_core::EventCategory::Build {
        score += 6;
    }
    if event.category == chronicle_core::EventCategory::Filesystem {
        score += 12;
    }
    if event.category == chronicle_core::EventCategory::Os && event.r#type == "process.focus" {
        let app = event
            .metadata
            .get("app_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if crate::summary_filter::is_terminal_container_app(app) && event.project.is_none() {
            return 0;
        }
    }
    if is_meaningful_failure(event) {
        score += 5;
    }
    if event.category == chronicle_core::EventCategory::Shell {
        if event
            .metadata
            .get("activity_label")
            .and_then(|v| v.as_str())
            .is_some()
        {
            score += 6;
        } else {
            score += 1;
        }
    }
    if event
        .metadata
        .get("activity_label")
        .and_then(|v| v.as_str())
        .is_some_and(|l| {
            l.contains("deploy") || l.contains("commit") || l.contains("push") || l.contains("test")
        })
    {
        score += 8;
    }
    if is_high_signal(event) {
        score += 2;
    }
    score
}

fn format_local_ts(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_else(|| "??:??".into())
}

fn format_local_date(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%A, %B %d, %Y")
                .to_string()
        })
        .unwrap_or_else(|| "today".into())
}
