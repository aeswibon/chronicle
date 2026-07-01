//! Rule-based fallback summaries (no LLM).

use chronicle_core::{CanonicalEvent, Span};
use std::collections::HashSet;

pub fn daily_summary(spans: &[Span], events: &[CanonicalEvent]) -> String {
    let mut projects: HashSet<String> = HashSet::new();
    let mut labels: HashSet<String> = HashSet::new();
    let mut errors = 0u32;
    let mut highlight_lines: Vec<String> = Vec::new();

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
        if let Some(label) = event
            .metadata
            .get("activity_label")
            .and_then(|v| v.as_str())
        {
            labels.insert(label.to_string());
        }
        if event.metadata.get("outcome").and_then(|v| v.as_str()) == Some("failure") {
            errors += 1;
            if let Some(line) = event.metadata.get("report_line").and_then(|v| v.as_str()) {
                if highlight_lines.len() < 3 {
                    highlight_lines.push(line.to_string());
                }
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
        parts.push(format!(
            "{errors} failed command{}",
            if errors == 1 { "" } else { "s" }
        ));
    }
    if !highlight_lines.is_empty() {
        parts.push(format!("Notable: {}", highlight_lines.join("; ")));
    }

    parts.join(". ") + "."
}
