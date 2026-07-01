//! Rule-based fallback summaries (no LLM).

use crate::summary_filter::{is_high_signal, is_meaningful_failure, is_summary_noise};
use chronicle_core::{CanonicalEvent, Span};
use std::collections::{HashMap, HashSet};

pub fn daily_summary(since: i64, spans: &[Span], events: &[CanonicalEvent]) -> String {
    let meaningful: Vec<&CanonicalEvent> = events.iter().filter(|e| !is_summary_noise(e)).collect();

    let focus_mins: u64 = spans
        .iter()
        .map(|s| s.duration_ms.unwrap_or(0))
        .sum::<u64>()
        / 60_000;

    let mut projects: HashMap<String, u32> = HashMap::new();
    for span in spans {
        if let Some(p) = &span.project {
            *projects.entry(p.clone()).or_insert(0) += 1;
        }
    }
    for event in &meaningful {
        if let Some(p) = &event.project {
            *projects.entry(p.clone()).or_insert(0) += 1;
        }
    }

    let mut project_names: Vec<_> = projects.into_iter().collect();
    project_names.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let project_names: Vec<String> = project_names
        .into_iter()
        .take(5)
        .map(|(name, _)| name)
        .collect();

    let git_events: Vec<_> = meaningful
        .iter()
        .filter(|e| e.category == chronicle_core::EventCategory::Git)
        .collect();

    let mut commits = 0u32;
    let mut pushes = 0u32;
    let mut merges = 0u32;
    for e in &git_events {
        match e.r#type.as_str() {
            "commit.created" => commits += 1,
            "push.completed" => pushes += 1,
            "merge.completed" => merges += 1,
            _ => {}
        }
    }

    let meaningful_failures: Vec<_> = meaningful
        .iter()
        .filter(|e| is_meaningful_failure(e))
        .collect();

    let date_label = format_local_date(since);
    let mut sentences = Vec::new();

    if focus_mins > 0 && !spans.is_empty() {
        sentences.push(format!(
            "On {date_label}, logged about {focus_mins} minutes across {} focus block{}.",
            spans.len(),
            if spans.len() == 1 { "" } else { "s" }
        ));
    } else if !meaningful.is_empty() {
        sentences.push(format!(
            "On {date_label}, captured developer activity across the day."
        ));
    } else {
        return format!("On {date_label}, no meaningful activity was recorded.");
    }

    if !project_names.is_empty() {
        let list = project_names.join(", ");
        sentences.push(format!(
            "Primary project{}: {list}.",
            if project_names.len() == 1 { "" } else { "s" }
        ));
    }

    if !git_events.is_empty() {
        let mut git_parts = Vec::new();
        if commits > 0 {
            git_parts.push(format!(
                "{commits} commit{}",
                if commits == 1 { "" } else { "s" }
            ));
        }
        if pushes > 0 {
            git_parts.push(format!(
                "{pushes} push{}",
                if pushes == 1 { "" } else { "es" }
            ));
        }
        if merges > 0 {
            git_parts.push(format!(
                "{merges} merge{}",
                if merges == 1 { "" } else { "s" }
            ));
        }
        if !git_parts.is_empty() {
            sentences.push(format!("Git activity: {}.", git_parts.join(", ")));
        }
    }

    let labels = collect_activity_labels(spans, &meaningful);
    if !labels.is_empty() {
        let capped: Vec<_> = labels.into_iter().take(6).collect();
        sentences.push(format!("Work themes: {}.", capped.join(", ")));
    }

    if meaningful.len() > 400 {
        sentences.push(format!(
            "Captured {} high-signal events ({} total in store for this day).",
            meaningful.len(),
            events.len()
        ));
    }

    if !meaningful_failures.is_empty() {
        let count = meaningful_failures.len();
        let sample = meaningful_failures
            .iter()
            .take(2)
            .filter_map(|e| e.metadata.get("report_line").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
            .join("; ");
        if sample.is_empty() {
            sentences.push(format!(
                "Noted {count} build or test failure{} worth revisiting.",
                if count == 1 { "" } else { "s" }
            ));
        } else {
            sentences.push(format!(
                "Noted {count} issue{}: {sample}.",
                if count == 1 { "" } else { "s" }
            ));
        }
    }

    sentences.join(" ")
}

fn collect_activity_labels(spans: &[Span], events: &[&CanonicalEvent]) -> Vec<String> {
    let mut labels: HashSet<String> = HashSet::new();
    for span in spans {
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
        if is_high_signal(event) {
            if let Some(label) = event
                .metadata
                .get("activity_label")
                .and_then(|v| v.as_str())
            {
                labels.insert(label.to_string());
            }
        }
    }
    let mut list: Vec<_> = labels.into_iter().collect();
    list.sort();
    list
}

fn format_local_date(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%A, %B %d")
                .to_string()
        })
        .unwrap_or_else(|| "today".into())
}
