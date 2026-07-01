//! Privacy sanitization and recording policy.

use chronicle_config::PrivacyConfig;
use chronicle_core::{CanonicalEvent, EventCategory};

/// Returns false for events that should not be stored or broadcast.
pub fn should_record(event: &CanonicalEvent) -> bool {
    let privacy = chronicle_config::load().privacy;
    match event.category {
        EventCategory::Os => should_record_focus(event),
        EventCategory::Shell => should_record_shell(event),
        EventCategory::Git => should_record_git(event),
        EventCategory::Filesystem => should_record_filesystem(event),
        EventCategory::Browser => should_record_browser(event, &privacy),
        _ => true,
    }
}

/// Redact secrets and strip sensitive URL parts before persistence.
pub fn sanitize_event(event: &mut CanonicalEvent) {
    let privacy = chronicle_config::load().privacy;
    if privacy.redact_shell_secrets && event.category == EventCategory::Shell {
        if let Some(cmd) = event
            .metadata
            .get_mut("command")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
        {
            let redacted = redact_command_secrets(&cmd);
            if let Some(obj) = event.metadata.as_object_mut() {
                obj.insert("command".into(), redacted.into());
            }
        }
    }
    if privacy.strip_query_params && event.category == EventCategory::Browser {
        strip_browser_query_params(event);
    }
}

fn should_record_browser(event: &CanonicalEvent, privacy: &PrivacyConfig) -> bool {
    if privacy.allowed_domains.is_empty() {
        return true;
    }
    let domain = event
        .metadata
        .get("domain")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if domain.is_empty() {
        return false;
    }
    privacy
        .allowed_domains
        .iter()
        .any(|allowed| domain == allowed || domain.ends_with(&format!(".{allowed}")))
}

fn strip_browser_query_params(event: &mut CanonicalEvent) {
    let Some(obj) = event.metadata.as_object_mut() else {
        return;
    };
    for key in ["url", "path"] {
        if let Some(value) = obj.get(key).and_then(|v| v.as_str()) {
            if let Ok(parsed) = url::Url::parse(value) {
                let mut clean = parsed;
                clean.set_query(None);
                clean.set_fragment(None);
                obj.insert(key.into(), clean.to_string().into());
            } else if key == "path" && value.contains('?') {
                let stripped = value.split('?').next().unwrap_or(value);
                obj.insert(key.into(), stripped.into());
            }
        }
    }
}

fn redact_command_secrets(cmd: &str) -> String {
    let mut out = cmd.to_string();
    const MARKERS: &[&str] = &[
        "API_KEY=",
        "TOKEN=",
        "SECRET=",
        "PASSWORD=",
        "PASSWD=",
        "BEARER ",
    ];
    for marker in MARKERS {
        if let Some(idx) = out.to_uppercase().find(&marker.to_uppercase()) {
            let tail = &out[idx + marker.len()..];
            let end = tail
                .find(|c: char| c.is_whitespace() || c == ';' || c == '&')
                .unwrap_or(tail.len());
            let range = idx + marker.len()..idx + marker.len() + end;
            out.replace_range(range, "***");
        }
    }
    out
}

fn should_record_focus(event: &CanonicalEvent) -> bool {
    if event.r#type != "process.focus" {
        return true;
    }

    let meta = event.metadata.as_object();
    let app = meta
        .and_then(|m| m.get("app_name"))
        .and_then(|v| v.as_str())
        .unwrap_or(&event.source);

    let app_lower = app.to_lowercase();
    const IGNORED_APPS: &[&str] = &[
        "chronicle-ui",
        "chronicle",
        "system settings",
        "notification centre",
        "control centre",
        "windowmanager",
        "loginwindow",
        "dock",
        "systemuiserver",
    ];

    !IGNORED_APPS.iter().any(|ignored| app_lower == *ignored)
}

fn should_record_shell(event: &CanonicalEvent) -> bool {
    let cmd = event
        .metadata
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or(&event.source);

    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return false;
    }

    let first = trimmed.split_whitespace().next().unwrap_or("");
    let base = first.rsplit('/').next().unwrap_or(first);

    const NOISE: &[&str] = &[
        "cd", "ls", "pwd", "clear", "echo", "exit", "fg", "bg", "jobs", "pushd", "popd", "dirs",
        "history", "which", "type", "true", "false", ":", "printf", "test", "[", "[[",
    ];

    !NOISE.contains(&base)
}

fn should_record_git(event: &CanonicalEvent) -> bool {
    event.r#type != "git.other"
}

fn should_record_filesystem(event: &CanonicalEvent) -> bool {
    matches!(event.r#type.as_str(), "file.created" | "file.deleted")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronicle_core::CanonicalEvent;

    #[test]
    fn skips_chronicle_focus() {
        let mut event = CanonicalEvent::new("chronicle-ui", EventCategory::Os, "process.focus");
        event.metadata = serde_json::json!({"app_name": "chronicle-ui"});
        assert!(!should_record(&event));
    }

    #[test]
    fn skips_shell_cd() {
        let mut event = CanonicalEvent::new("zsh", EventCategory::Shell, "command.completed");
        event.metadata = serde_json::json!({"command": "cd ~/Developer"});
        assert!(!should_record(&event));
    }

    #[test]
    fn keeps_git_commit() {
        let event = CanonicalEvent::new("git", EventCategory::Git, "commit.created");
        assert!(should_record(&event));
    }

    #[test]
    fn skips_file_modified() {
        let event = CanonicalEvent::new("fs", EventCategory::Filesystem, "file.modified");
        assert!(!should_record(&event));
    }

    #[test]
    fn redacts_api_key_in_command() {
        let mut event = CanonicalEvent::new("zsh", EventCategory::Shell, "command.completed");
        event.metadata =
            serde_json::json!({"command": "curl -H API_KEY=sk-secret123 https://api.example.com"});
        sanitize_event(&mut event);
        let cmd = event.metadata["command"].as_str().unwrap();
        assert!(!cmd.contains("sk-secret"));
        assert!(cmd.contains("***"));
    }
}
