//! Filters for daily summaries — exclude terminal noise, rank meaningful work.

use chronicle_core::{CanonicalEvent, EventCategory, Span, SpanType};
use std::collections::HashMap;

/// Events that should not influence daily rollups or highlight selection.
pub fn is_summary_noise(event: &CanonicalEvent) -> bool {
    match event.category {
        EventCategory::Shell => is_shell_noise(event),
        EventCategory::Os => {
            event.r#type == "process.focus"
                && event
                    .metadata
                    .get("app_name")
                    .and_then(|v| v.as_str())
                    .is_some_and(|a| {
                        let lower = a.to_lowercase();
                        lower.contains("chronicle") || lower == "finder"
                    })
        }
        _ => false,
    }
}

/// Shell failures worth mentioning (build/test), not exploratory typos or git auth.
pub fn is_meaningful_failure(event: &CanonicalEvent) -> bool {
    if event.metadata.get("outcome").and_then(|v| v.as_str()) != Some("failure") {
        return false;
    }
    if event.category != EventCategory::Shell {
        return false;
    }
    let cmd = event
        .metadata
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    if is_shell_noise_cmd(&cmd) {
        return false;
    }
    const SIGNAL: &[&str] = &[
        "cargo",
        "npm",
        "pnpm",
        "bun ",
        "go test",
        "go build",
        "make",
        "cmake",
        "pytest",
        "jest",
        "vitest",
        "docker",
        "kubectl",
        "terraform",
        "fly deploy",
        "vercel",
        "tauri build",
    ];
    SIGNAL.iter().any(|s| cmd.contains(s))
}

pub fn is_high_signal(event: &CanonicalEvent) -> bool {
    if is_summary_noise(event) {
        return false;
    }
    match event.category {
        EventCategory::Git => matches!(
            event.r#type.as_str(),
            "commit.created" | "merge.completed" | "push.completed" | "rebase.completed"
        ),
        EventCategory::Ide | EventCategory::Build => true,
        EventCategory::Shell => event
            .metadata
            .get("activity_label")
            .and_then(|v| v.as_str())
            .is_some_and(|l| !is_generic_theme(l)),
        EventCategory::Os => event.r#type == "process.focus",
        _ => false,
    }
}

/// Parent-folder or workspace roots that are not useful project names in rollups.
pub fn is_generic_project(name: &str) -> bool {
    matches!(
        name,
        "developer" | "personal" | "tmp" | "github" | "Volumes" | "Seagate" | "Users" | "home"
    )
}

/// Low-signal activity labels that should not appear in daily prose.
pub fn is_generic_theme(label: &str) -> bool {
    matches!(
        label,
        "branch switch"
            | "commit"
            | "push"
            | "pull"
            | "fetch"
            | "commit workflow"
            | "terminal"
            | "focus"
            | "coding"
            | "editing"
            | "ide activity"
            | "browser activity"
            | "browsing"
            | "research"
    )
}

pub fn theme_priority(label: &str) -> i32 {
    if label.contains("session") || label.contains("iteration") {
        10
    } else if label.contains("deploy") || label.contains("integration") {
        8
    } else if label.contains("debug") || label.contains("agent") {
        7
    } else if label.contains("build") {
        5
    } else {
        1
    }
}

/// Rank projects by focused time plus high-signal events (milliseconds score).
pub fn rank_projects(spans: &[Span], events: &[&CanonicalEvent]) -> Vec<(String, u64)> {
    let mut scores: HashMap<String, u64> = HashMap::new();
    for span in spans {
        if span.span_type == SpanType::Idle {
            continue;
        }
        let Some(project) = span.project.as_ref() else {
            continue;
        };
        if is_generic_project(project) {
            continue;
        }
        *scores.entry(project.clone()).or_insert(0) += span.duration_ms.unwrap_or(0).max(60_000);
    }
    for event in events {
        if !is_high_signal(event) {
            continue;
        }
        let Some(project) = event.project.as_ref() else {
            continue;
        };
        if is_generic_project(project) {
            continue;
        }
        *scores.entry(project.clone()).or_insert(0) += 120_000;
    }
    let mut ranked: Vec<_> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
}

fn is_shell_noise(event: &CanonicalEvent) -> bool {
    let cmd = event
        .metadata
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    is_shell_noise_cmd(cmd)
}

fn is_shell_noise_cmd(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return true;
    }
    if trimmed.contains("| pbcopy")
        || trimmed.contains("| pbpaste")
        || trimmed.contains("pbcopy")
        || trimmed.contains("pbpaste")
    {
        return true;
    }

    let first = trimmed.split_whitespace().next().unwrap_or("");
    let base = first.rsplit('/').next().unwrap_or(first);

    const NOISE: &[&str] = &[
        "cd", "ls", "pwd", "clear", "echo", "exit", "fg", "bg", "jobs", "pushd", "popd", "dirs",
        "history", "which", "type", "true", "false", ":", "printf", "test", "[", "[[", "cat",
        "head", "tail", "wc", "less", "more", "open", "xargs", "tee", "touch", "mkdir", "rmdir",
        "mv", "cp", "chmod", "chown", "stat", "file", "du", "df", "env", "export", "unset",
        "alias", "unalias", "source", "builtin", "command", "hash", "sleep", "date", "cal",
    ];

    if NOISE.contains(&base) {
        return true;
    }

    if base == "rm" {
        let lower = trimmed.to_lowercase();
        return lower.contains("~/.")
            || lower.contains("/.cursor")
            || lower.contains("/.agent")
            || lower.contains("node_modules");
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronicle_core::CanonicalEvent;

    #[test]
    fn filters_pbcopy_pipe() {
        let mut e = CanonicalEvent::new("zsh", EventCategory::Shell, "command.failed");
        e.metadata = serde_json::json!({"command": "cat foo.md | pbcopy"});
        assert!(is_summary_noise(&e));
    }

    #[test]
    fn keeps_git_push() {
        let e = CanonicalEvent::new("git", EventCategory::Git, "push.completed");
        assert!(is_high_signal(&e));
    }

    #[test]
    fn git_checkout_not_high_signal() {
        let e = CanonicalEvent::new("git", EventCategory::Git, "branch.checkout");
        assert!(!is_high_signal(&e));
    }

    #[test]
    fn git_failure_not_meaningful() {
        let mut e = CanonicalEvent::new("git", EventCategory::Git, "push.failed");
        e.metadata = serde_json::json!({"outcome": "failure"});
        assert!(!is_meaningful_failure(&e));
    }
}
