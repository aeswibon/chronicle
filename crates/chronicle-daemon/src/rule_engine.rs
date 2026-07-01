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
        let end = span.ended_at.unwrap_or_else(|| {
            span.metadata
                .get("last_event_at")
                .and_then(|v| v.as_i64())
                .unwrap_or_else(|| chrono::Utc::now().timestamp_millis())
        });
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
        if singles.contains(&"agent session") {
            compound.push("agent session");
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
        EventCategory::Ide => ide_label(event),
        EventCategory::Browser => browser_label(event),
        EventCategory::Os => os_label(event),
        EventCategory::Build => Some("build"),
        _ => None,
    }
}

fn os_label(event: &CanonicalEvent) -> Option<&'static str> {
    match event.r#type.as_str() {
        "process.focus" => process_focus_label(event),
        "window.focus" => window_focus_label(event),
        _ => None,
    }
}

fn process_focus_label(event: &CanonicalEvent) -> Option<&'static str> {
    app_context_label(event)
}

fn window_focus_label(event: &CanonicalEvent) -> Option<&'static str> {
    if is_browser_app(event) {
        Some("browsing")
    } else if is_finder_app(event) {
        Some("files")
    } else {
        None
    }
}

fn app_context_label(event: &CanonicalEvent) -> Option<&'static str> {
    if is_agent_app(event) {
        return Some("agent session");
    }
    if is_terminal_app(event) {
        return Some("terminal");
    }
    if is_ide_app(event) {
        return Some("coding");
    }
    if is_browser_app(event) {
        return Some("browsing");
    }
    if is_finder_app(event) {
        return Some("files");
    }
    None
}

fn is_agent_app(event: &CanonicalEvent) -> bool {
    let app = event
        .metadata
        .get("app_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&event.source)
        .to_lowercase();
    let bundle = event
        .metadata
        .get("bundle_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    [
        "cursor",
        "claude",
        "codex",
        "gemini",
        "windsurf",
        "copilot",
        "aider",
        "opencode",
        "antigravity",
    ]
    .iter()
    .any(|n| app.contains(n) || bundle.contains(n))
}

fn is_terminal_app(event: &CanonicalEvent) -> bool {
    let app = event
        .metadata
        .get("app_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&event.source)
        .to_lowercase();
    [
        "terminal",
        "iterm",
        "warp",
        "ghostty",
        "alacritty",
        "kitty",
        "wezterm",
    ]
    .iter()
    .any(|n| app.contains(n))
}

fn is_ide_app(event: &CanonicalEvent) -> bool {
    let app = event
        .metadata
        .get("app_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&event.source)
        .to_lowercase();
    app.contains("xcode")
        || app.contains("intellij")
        || app.contains("android studio")
        || (app.contains("code") && !app.contains("cursor"))
}

fn is_browser_app(event: &CanonicalEvent) -> bool {
    let app = event
        .metadata
        .get("app_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&event.source)
        .to_lowercase();
    let bundle = event
        .metadata
        .get("bundle_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    [
        "safari", "chrome", "firefox", "brave", "arc", "edge", "opera", "vivaldi", "chromium",
    ]
    .iter()
    .any(|n| app.contains(n) || bundle.contains(n))
}

fn is_finder_app(event: &CanonicalEvent) -> bool {
    let app = event
        .metadata
        .get("app_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&event.source)
        .to_lowercase();
    let bundle = event
        .metadata
        .get("bundle_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    app.contains("finder") || bundle.contains("com.apple.finder")
}

fn shell_label(event: &CanonicalEvent) -> Option<&'static str> {
    let cmd = event.metadata.get("command")?.as_str()?.to_lowercase();
    if cmd.contains("git push") {
        Some("push")
    } else if cmd.contains("git commit") {
        Some("commit")
    } else if cmd.contains("git merge") {
        Some("merge")
    } else if cmd.contains("git pull") {
        Some("pull")
    } else if matches_test_cmd(&cmd) {
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
        "push.completed" => Some("push"),
        "pull.completed" | "fetch.completed" => None,
        _ => None,
    }
}

fn ide_label(event: &CanonicalEvent) -> Option<&'static str> {
    match event.r#type.as_str() {
        t if t.starts_with("ide.test") => Some("test iteration"),
        t if t.starts_with("ide.debug") => Some("debugging"),
        t if t.contains("save") => Some("editing"),
        t if t.contains("focus") => Some("coding"),
        _ => Some("ide activity"),
    }
}

fn browser_label(event: &CanonicalEvent) -> Option<&'static str> {
    match event.r#type.as_str() {
        "page.focus" => Some("research"),
        "page.navigate" => Some("browsing"),
        _ => Some("browser activity"),
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
    fn labels_shell_git_push() {
        let mut e = CanonicalEvent::new("zsh", EventCategory::Shell, "command.completed");
        e.metadata = json!({"command": "git push origin master"});
        annotate_event(&mut e);
        assert_eq!(
            e.metadata.get("activity_label").and_then(|v| v.as_str()),
            Some("push")
        );
    }

    #[test]
    fn labels_no_generic_focus_for_notes() {
        let mut e = CanonicalEvent::new("Notes", EventCategory::Os, "process.focus");
        e.metadata = json!({"app_name": "Notes", "bundle_id": "com.apple.Notes"});
        annotate_event(&mut e);
        assert!(e.metadata.get("activity_label").is_none());
    }

    #[test]
    fn labels_finder_window_switch() {
        let mut e = CanonicalEvent::new("Finder", EventCategory::Os, "window.focus");
        e.metadata = json!({"app_name": "Finder", "bundle_id": "com.apple.finder", "window_title": "Downloads"});
        annotate_event(&mut e);
        assert_eq!(
            e.metadata.get("activity_label").and_then(|v| v.as_str()),
            Some("files")
        );
    }

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
    fn labels_git_push() {
        let mut e = CanonicalEvent::new("git", EventCategory::Git, "push.completed");
        annotate_event(&mut e);
        assert_eq!(
            e.metadata.get("activity_label").and_then(|v| v.as_str()),
            Some("push")
        );
    }

    #[test]
    fn labels_ide_test() {
        let mut e = CanonicalEvent::new("vscode", EventCategory::Ide, "ide.test.run");
        annotate_event(&mut e);
        assert_eq!(
            e.metadata.get("activity_label").and_then(|v| v.as_str()),
            Some("test iteration")
        );
    }
}
