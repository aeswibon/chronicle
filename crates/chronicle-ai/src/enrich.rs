//! Deterministic metadata enrichment for AI-readable activity reports.

use chronicle_core::{CanonicalEvent, EventCategory};

/// Add `report_line`, `intent`, `tool`, and `outcome` to event metadata.
pub fn enrich_event(event: &mut CanonicalEvent) {
    let intent = detect_intent(event);
    let tool = detect_tool(event);
    let outcome = detect_outcome(event);
    let report_line = build_report_line(event, intent, tool, outcome);

    let Some(obj) = event.metadata.as_object_mut() else {
        return;
    };
    obj.insert("intent".into(), intent.into());
    if let Some(tool) = tool {
        obj.insert("tool".into(), tool.into());
    }
    obj.insert("outcome".into(), outcome.into());
    obj.insert("report_line".into(), report_line.into());
}

fn detect_intent(event: &CanonicalEvent) -> &'static str {
    if let Some(label) = event
        .metadata
        .get("activity_label")
        .and_then(|v| v.as_str())
    {
        return match label {
            "test iteration" => "testing",
            "debugging" => "debugging",
            "deployment" => "deploy",
            "build" => "build",
            "commit" | "merge" | "push" | "rebase" | "branch switch" => "git",
            "research" | "browsing" => "research",
            "coding" | "editing" | "ide activity" => "coding",
            _ => "work",
        };
    }
    match event.category {
        EventCategory::Os => "app",
        EventCategory::Shell => "terminal",
        EventCategory::Git => "git",
        EventCategory::Filesystem => "filesystem",
        EventCategory::Ide => "coding",
        EventCategory::Browser => "research",
        EventCategory::Build => "build",
        EventCategory::Ai => "ai",
        _ => "work",
    }
}

fn detect_tool(event: &CanonicalEvent) -> Option<&'static str> {
    match event.category {
        EventCategory::Shell => shell_tool(event),
        EventCategory::Git => Some("git"),
        EventCategory::Ide => Some("ide"),
        EventCategory::Browser => Some("browser"),
        EventCategory::Os => Some("macos"),
        EventCategory::Build => Some("build"),
        _ => None,
    }
}

fn shell_tool(event: &CanonicalEvent) -> Option<&'static str> {
    let cmd = event.metadata.get("command")?.as_str()?.to_lowercase();
    if cmd.contains("cargo") {
        Some("cargo")
    } else if cmd.contains("npm") || cmd.contains("pnpm") || cmd.contains("bun") {
        Some("node")
    } else if cmd.contains("go ") {
        Some("go")
    } else if cmd.contains("docker") {
        Some("docker")
    } else if cmd.contains("kubectl") || cmd.contains("helm") {
        Some("k8s")
    } else if cmd.contains("git ") {
        Some("git")
    } else if cmd.contains("make") {
        Some("make")
    } else {
        Some("shell")
    }
}

fn detect_outcome(event: &CanonicalEvent) -> &'static str {
    if event.r#type.contains("failed") {
        return "failure";
    }
    if let Some(code) = event.metadata.get("exit_code").and_then(|v| v.as_str()) {
        return if code == "0" { "success" } else { "failure" };
    }
    "neutral"
}

fn build_report_line(
    event: &CanonicalEvent,
    intent: &str,
    tool: Option<&str>,
    outcome: &str,
) -> String {
    let project = event
        .project
        .as_deref()
        .map(|p| format!(" in {p}"))
        .unwrap_or_default();
    let tool_prefix = tool.map(|t| format!("{t}: ")).unwrap_or_default();

    match event.category {
        EventCategory::Shell => {
            let cmd = event
                .metadata
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("command");
            let dur = event
                .duration_ms
                .map(|ms| format!(", {:.0}s", ms as f64 / 1000.0))
                .unwrap_or_default();
            let status = if outcome == "failure" {
                format!(
                    " (exit {})",
                    event
                        .metadata
                        .get("exit_code")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                )
            } else {
                String::new()
            };
            format!("{tool_prefix}{cmd}{project}{dur}{status}")
        }
        EventCategory::Git => {
            let detail = event
                .metadata
                .get("reflog")
                .or_else(|| event.metadata.get("branch"))
                .and_then(|v| v.as_str())
                .unwrap_or(&event.r#type);
            format!("git {detail}{project}")
        }
        EventCategory::Os => {
            let app = event
                .metadata
                .get("app_name")
                .and_then(|v| v.as_str())
                .unwrap_or(&event.source);
            let title = event
                .metadata
                .get("window_title")
                .and_then(|v| v.as_str())
                .map(|t| format!(" — {t}"))
                .unwrap_or_default();
            let agent = is_agent_app_name(app);
            if event.r#type == "window.focus" {
                if title.is_empty() {
                    format!("Window changed in {app}{project}")
                } else {
                    format!("Window in {app}{title}{project}")
                }
            } else if agent {
                format!("{app}{title}{project}")
            } else if title.is_empty() {
                format!("{app}{project}")
            } else {
                format!("{app}{title}{project}")
            }
        }
        EventCategory::Ide => {
            let file = event
                .metadata
                .get("file")
                .and_then(|v| v.as_str())
                .map(file_basename)
                .unwrap_or_else(|| event.r#type.replace('.', " "));
            format!("IDE {intent}: {file}{project}")
        }
        EventCategory::Browser => {
            let domain = event
                .metadata
                .get("domain")
                .and_then(|v| v.as_str())
                .unwrap_or("page");
            let title = event
                .metadata
                .get("title")
                .and_then(|v| v.as_str())
                .map(|t| format!(" — {t}"))
                .unwrap_or_default();
            format!("Browser {domain}{title}")
        }
        EventCategory::Filesystem => {
            let path = event
                .metadata
                .get("path")
                .and_then(|v| v.as_str())
                .map(file_basename)
                .unwrap_or_else(|| "file".to_string());
            format!("File {}: {path}{project}", event.r#type)
        }
        _ => format!("{tool_prefix}{}{project}", event.r#type),
    }
}

fn file_basename(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn is_agent_app_name(app: &str) -> bool {
    let lower = app.to_lowercase();
    [
        "cursor", "claude", "codex", "gemini", "windsurf", "copilot", "aider", "opencode",
    ]
    .iter()
    .any(|n| lower.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronicle_core::CanonicalEvent;
    use serde_json::json;

    #[test]
    fn enriches_os_without_focused_prefix() {
        let mut e = CanonicalEvent::new("Ghostty", EventCategory::Os, "process.focus");
        e.metadata = json!({"app_name": "Ghostty"});
        enrich_event(&mut e);
        let line = e.metadata["report_line"].as_str().unwrap();
        assert!(!line.to_lowercase().contains("focused"));
        assert!(!line.to_lowercase().contains("switched"));
        assert!(line.contains("Ghostty"));
    }

    fn enriches_shell_failure() {
        let mut e = CanonicalEvent::new("cargo", EventCategory::Shell, "command.failed");
        e.project = Some("chronicle".into());
        e.metadata = json!({"command": "cargo test", "exit_code": "101"});
        enrich_event(&mut e);
        let line = e.metadata["report_line"].as_str().unwrap();
        assert!(line.contains("cargo test"));
        assert!(line.contains("chronicle"));
        assert_eq!(e.metadata["intent"].as_str(), Some("terminal"));
        assert_eq!(e.metadata["outcome"].as_str(), Some("failure"));
    }
}
