//! Structured digest for LLM daily summaries.

use crate::summary_filter::{is_high_signal, is_meaningful_failure, is_summary_noise};
use chronicle_core::{CanonicalEvent, Span};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize)]
pub struct DayReportContext {
    pub since: i64,
    pub until: i64,
    pub stats: DayStats,
    pub highlights: Vec<HighlightLine>,
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
        Self {
            since,
            until,
            stats,
            highlights,
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

fn compute_stats(spans: &[Span], events: &[CanonicalEvent]) -> DayStats {
    let mut projects: HashSet<String> = HashSet::new();
    let mut intents: HashSet<String> = HashSet::new();
    let mut error_count = 0u32;
    let mut focus_ms = 0u64;

    for span in spans {
        if let Some(p) = &span.project {
            projects.insert(p.clone());
        }
        focus_ms += span.duration_ms.unwrap_or(0);
    }

    for event in events {
        if let Some(p) = &event.project {
            projects.insert(p.clone());
        }
        if let Some(i) = event.metadata.get("intent").and_then(|v| v.as_str()) {
            intents.insert(i.to_string());
        }
        if is_meaningful_failure(event) {
            error_count += 1;
        }
    }

    let mut project_list: Vec<_> = projects.into_iter().collect();
    project_list.sort();
    let mut intent_list: Vec<_> = intents.into_iter().collect();
    intent_list.sort();

    DayStats {
        span_count: spans.len(),
        event_count: events.len(),
        project_count: project_list.len(),
        error_count,
        projects: project_list,
        intents: intent_list,
        focus_minutes: focus_ms / 60_000,
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
