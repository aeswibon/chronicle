//! Rule-based fallback summaries (no LLM).

use crate::context::DayReportContext;
use crate::summary_filter::{
    is_generic_project, is_generic_theme, is_high_signal, is_meaningful_failure, is_summary_noise,
    rank_projects, theme_priority,
};
use chronicle_core::{CanonicalEvent, Span, SpanType};
use std::collections::HashSet;

pub fn daily_summary(since: i64, until: i64, spans: &[Span], events: &[CanonicalEvent]) -> String {
    let meaningful: Vec<&CanonicalEvent> = events.iter().filter(|e| !is_summary_noise(e)).collect();

    let active_spans: Vec<&Span> = spans
        .iter()
        .filter(|s| s.span_type != SpanType::Idle)
        .collect();

    let focus_mins: u64 = active_spans
        .iter()
        .map(|s| s.duration_ms.unwrap_or(0))
        .sum::<u64>()
        / 60_000;
    let focus_mins = focus_mins.min(12 * 60);

    let ranked = rank_projects(spans, &meaningful);
    let date_label = format_local_date(since);
    let ctx = DayReportContext::build(since, until, spans, events);

    let mut sentences = Vec::new();

    if focus_mins > 0 && !active_spans.is_empty() {
        sentences.push(format!(
            "On {date_label}, {} across {} focus session{}.",
            format_focus_duration(focus_mins),
            active_spans.len(),
            if active_spans.len() == 1 { "" } else { "s" }
        ));
    } else if !meaningful.is_empty() {
        sentences.push(format!(
            "On {date_label}, captured developer activity across the day."
        ));
    } else {
        return format!("On {date_label}, no meaningful activity was recorded.");
    }

    if let Some(line) = format_projects_line(&ranked) {
        sentences.push(line);
    }

    if let Some(line) = format_git_line(&meaningful) {
        sentences.push(line);
    }

    let themes = collect_meaningful_themes(spans, &meaningful);
    if !themes.is_empty() {
        sentences.push(format!("Work themes: {}.", themes.join(", ")));
    } else if !ctx.highlights.is_empty() {
        let notable: Vec<String> = ctx
            .highlights
            .iter()
            .rev()
            .take(3)
            .map(|h| shorten_line(&h.line))
            .collect();
        if !notable.is_empty() {
            sentences.push(format!("Notable: {}.", join_natural(&notable)));
        }
    }

    let failures: Vec<_> = meaningful
        .iter()
        .filter(|e| is_summary_failure(e))
        .collect();
    if let Some(line) = format_failures_line(&failures) {
        sentences.push(line);
    }

    sentences.join(" ")
}

fn format_focus_duration(mins: u64) -> String {
    if mins >= 90 {
        let hours = (mins as f64 / 60.0 * 10.0).round() / 10.0;
        if (hours - hours.round()).abs() < f64::EPSILON {
            format!("logged about {} hours", hours.round() as u64)
        } else {
            format!("logged about {hours} hours")
        }
    } else if mins == 0 {
        "logged brief activity".into()
    } else {
        format!("logged about {mins} minutes")
    }
}

fn format_projects_line(ranked: &[(String, u64)]) -> Option<String> {
    if ranked.is_empty() {
        return None;
    }
    let top: Vec<_> = ranked.iter().take(3).collect();
    let rest = ranked.len().saturating_sub(3);
    let parts: Vec<String> = top
        .iter()
        .map(|(name, ms)| {
            if *ms >= 3_600_000 {
                format!("{name} (~{}h)", ms / 3_600_000)
            } else if *ms >= 60_000 {
                format!("{name} (~{}m)", ms / 60_000)
            } else {
                (*name).clone()
            }
        })
        .collect();

    Some(match (parts.len(), rest) {
        (1, 0) => format!("Focused on {}.", parts[0]),
        (_, 0) => format!("Focused mainly on {}.", join_natural(&parts)),
        (_, extra) => format!(
            "Focused mainly on {}; lighter work across {extra} other repo{}.",
            join_natural(&parts),
            if extra == 1 { "" } else { "s" }
        ),
    })
}

fn format_git_line(events: &[&CanonicalEvent]) -> Option<String> {
    let git_events: Vec<_> = events
        .iter()
        .filter(|e| e.category == chronicle_core::EventCategory::Git)
        .collect();
    if git_events.is_empty() {
        return None;
    }

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
    if git_parts.is_empty() {
        return None;
    }

    let repo_count = git_events
        .iter()
        .filter_map(|e| e.project.as_deref())
        .filter(|p| !is_generic_project(p))
        .collect::<HashSet<_>>()
        .len();
    if repo_count > 3 {
        Some(format!(
            "Git activity: {} across {repo_count} repos.",
            git_parts.join(", ")
        ))
    } else {
        Some(format!("Git activity: {}.", git_parts.join(", ")))
    }
}

fn collect_meaningful_themes(spans: &[Span], events: &[&CanonicalEvent]) -> Vec<String> {
    let mut labels: HashSet<String> = HashSet::new();
    for span in spans {
        if let Some(obj) = span.metadata.as_object() {
            if let Some(arr) = obj.get("activity_labels").and_then(|v| v.as_array()) {
                for label in arr {
                    if let Some(s) = label.as_str() {
                        if !is_generic_theme(s) {
                            labels.insert(s.to_string());
                        }
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
                if !is_generic_theme(label) {
                    labels.insert(label.to_string());
                }
            }
        }
    }

    let mut list: Vec<_> = labels.into_iter().collect();
    list.sort_by(|a, b| {
        theme_priority(b)
            .cmp(&theme_priority(a))
            .then_with(|| a.cmp(b))
    });
    list.truncate(4);
    list
}

fn is_summary_failure(event: &&CanonicalEvent) -> bool {
    if !is_meaningful_failure(event) {
        return false;
    }
    if event.category == chronicle_core::EventCategory::Shell {
        let cmd = event
            .metadata
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if cmd.len() > 180 {
            return false;
        }
    }
    true
}

fn format_failures_line(failures: &[&&CanonicalEvent]) -> Option<String> {
    if failures.is_empty() {
        return None;
    }
    let count = failures.len();
    let sample: Vec<String> = failures
        .iter()
        .take(2)
        .map(|e| {
            let project = e.project.as_deref().unwrap_or("project");
            if let Some(line) = e.metadata.get("report_line").and_then(|v| v.as_str()) {
                format!("{} ({project})", shorten_line(line))
            } else {
                format!("build/test failure in {project}")
            }
        })
        .collect();
    Some(format!(
        "Noted {count} build or test issue{}: {}.",
        if count == 1 { "" } else { "s" },
        sample.join("; ")
    ))
}

fn shorten_line(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.len() <= 72 {
        return trimmed.to_string();
    }
    let mut end = 72;
    while end > 40 && !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &trimmed[..end])
}

fn join_natural(parts: &[String]) -> String {
    match parts.len() {
        0 => String::new(),
        1 => parts[0].clone(),
        2 => format!("{} and {}", parts[0], parts[1]),
        _ => {
            let head = parts[..parts.len() - 1].join(", ");
            format!("{head}, and {}", parts.last().unwrap())
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use chronicle_core::{EventCategory, SpanType};
    use serde_json::json;

    fn span(project: &str, mins: u64) -> Span {
        let mut s = Span::new(SpanType::Coding, Some(project.into()));
        s.duration_ms = Some(mins * 60_000);
        s.ended_at = Some(s.started_at + (mins * 60_000) as i64);
        s
    }

    #[test]
    fn caps_projects_and_skips_generic_names() {
        let spans = vec![
            span("chronicle", 120),
            span("agent-brain", 45),
            span("developer", 30),
        ];
        let events: Vec<CanonicalEvent> = (0..5)
            .map(|_| CanonicalEvent::new("git", EventCategory::Git, "commit.created"))
            .collect();
        let text = daily_summary(0, 86_400_000, &spans, &events);
        assert!(text.contains("chronicle"));
        assert!(text.contains("agent-brain"));
        assert!(!text.contains("developer"));
        assert!(!text.contains("Primary projects:"));
    }

    #[test]
    fn omits_generic_git_themes() {
        let mut e = CanonicalEvent::new("git", EventCategory::Git, "branch.checkout");
        e.metadata = json!({"activity_label": "branch switch"});
        let text = daily_summary(0, 86_400_000, &[], std::slice::from_ref(&e));
        assert!(!text.contains("Work themes: branch switch"));
    }

    #[test]
    fn ignores_git_push_failures() {
        let mut e = CanonicalEvent::new("git", EventCategory::Git, "push.failed");
        e.metadata = json!({"outcome": "failure", "report_line": "git push in social-presence"});
        let text = daily_summary(0, 86_400_000, &[], std::slice::from_ref(&e));
        assert!(!text.contains("Noted"));
    }
}
