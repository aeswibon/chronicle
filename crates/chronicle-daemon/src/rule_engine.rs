//! Deterministic activity labels from shell and git events (no AI).

use chronicle_core::{CanonicalEvent, EventCategory, Span};
use serde_json::json;
use std::collections::HashSet;

const WINDOW_MS: i64 = 30 * 60 * 1000;

pub struct RuleEngine {
    recent: Vec<CanonicalEvent>,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self { recent: Vec::new() }
    }

    pub fn process(&mut self, event: &mut CanonicalEvent) {
        annotate_event(event);
        self.recent.push(event.clone());
        let cutoff = event.timestamp - WINDOW_MS;
        self.recent.retain(|e| e.timestamp >= cutoff);
    }

    pub fn annotate_span(&self, span: &mut Span) {
        let labels = self.span_activity_labels(span);
        if labels.is_empty() {
            return;
        }
        if let Some(obj) = span.metadata.as_object_mut() {
            obj.insert("activity_labels".into(), json!(labels));
        }
    }

    fn span_activity_labels(&self, span: &Span) -> Vec<&'static str> {
        let end = span.ended_at.unwrap_or(span.started_at);
        let in_span: Vec<&CanonicalEvent> = self
            .recent
            .iter()
            .filter(|e| e.timestamp >= span.started_at && e.timestamp <= end)
            .filter(|e| span.project.is_none() || e.project == span.project)
            .collect();

        let mut singles: HashSet<&'static str> = HashSet::new();
        for e in &in_span {
            if let Some(label) = single_event_label(e) {
                singles.insert(label);
            }
        }

        let mut compound = Vec::new();
        if singles.contains(&"deployment") || has_deploy_command(&in_span) {
            compound.push("deployment investigation");
        }
        if singles.contains(&"test iteration") && singles.contains(&"debugging") {
            compound.push("debugging session");
        } else if singles.contains(&"test iteration") {
            compound.push("test iteration");
        }
        if singles.contains(&"commit") && singles.contains(&"merge") {
            compound.push("integration");
        } else if singles.contains(&"commit") {
            compound.push("commit workflow");
        }

        if compound.is_empty() {
            singles.into_iter().collect()
        } else {
            compound
        }
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub fn annotate_event(event: &mut CanonicalEvent) {
    let Some(label) = single_event_label(event) else {
        return;
    };
    if let Some(obj) = event.metadata.as_object_mut() {
        obj.insert("activity_label".into(), json!(label));
    }
}

fn single_event_label(event: &CanonicalEvent) -> Option<&'static str> {
    match event.category {
        EventCategory::Shell => shell_label(event),
        EventCategory::Git => git_label(event),
        _ => None,
    }
}

fn shell_label(event: &CanonicalEvent) -> Option<&'static str> {
    let cmd = event.metadata.get("command")?.as_str()?.to_lowercase();
    if matches_test_cmd(&cmd) {
        Some("test iteration")
    } else if matches_debug_cmd(&cmd) {
        Some("debugging")
    } else if matches_deploy_cmd(&cmd) {
        Some("deployment")
    } else if matches_build_cmd(&cmd) {
        Some("build")
    } else {
        None
    }
}

fn git_label(event: &CanonicalEvent) -> Option<&'static str> {
    match event.r#type.as_str() {
        "commit.created" => Some("commit"),
        "merge.completed" => Some("merge"),
        "branch.checkout" => Some("branch switch"),
        "rebase.completed" => Some("rebase"),
        _ => None,
    }
}

fn has_deploy_command(events: &[&CanonicalEvent]) -> bool {
    events.iter().any(|e| {
        e.metadata
            .get("command")
            .and_then(|v| v.as_str())
            .is_some_and(|c| matches_deploy_cmd(&c.to_lowercase()))
    })
}

fn matches_test_cmd(cmd: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "cargo test",
        "go test",
        "pytest",
        "npm test",
        "pnpm test",
        "bun test",
        "jest",
        "vitest",
        "mocha",
        "rspec",
        "phpunit",
    ];
    NEEDLES.iter().any(|n| cmd.contains(n))
}

fn matches_debug_cmd(cmd: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "lldb",
        "gdb",
        "dlv ",
        "debug",
        "cargo run",
        "go run",
        "node --inspect",
    ];
    NEEDLES.iter().any(|n| cmd.contains(n))
}

fn matches_deploy_cmd(cmd: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "kubectl",
        "helm ",
        "docker compose",
        "docker-compose",
        "terraform apply",
        "pulumi up",
        "fly deploy",
        "vercel deploy",
        "railway up",
    ];
    NEEDLES.iter().any(|n| cmd.contains(n))
}

fn matches_build_cmd(cmd: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "cargo build",
        "cargo check",
        "npm run build",
        "pnpm build",
        "bun run build",
        "go build",
        "make ",
        "cmake ",
        "tsc",
        "eslint",
    ];
    NEEDLES.iter().any(|n| cmd.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronicle_core::CanonicalEvent;

    #[test]
    fn labels_cargo_test() {
        let mut e = CanonicalEvent::new("cargo", EventCategory::Shell, "command.completed");
        e.metadata = json!({"command": "cargo test -p chronicle-core"});
        annotate_event(&mut e);
        assert_eq!(
            e.metadata.get("activity_label").and_then(|v| v.as_str()),
            Some("test iteration")
        );
    }

    #[test]
    fn labels_git_commit() {
        let mut e = CanonicalEvent::new("git", EventCategory::Git, "commit.created");
        annotate_event(&mut e);
        assert_eq!(
            e.metadata.get("activity_label").and_then(|v| v.as_str()),
            Some("commit")
        );
    }
}
