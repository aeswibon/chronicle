//! Filters for daily summaries — exclude terminal noise, rank meaningful work.

use chronicle_core::{CanonicalEvent, EventCategory};

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

/// Shell failures worth mentioning (build/test/git), not exploratory typos.
pub fn is_meaningful_failure(event: &CanonicalEvent) -> bool {
    if event.metadata.get("outcome").and_then(|v| v.as_str()) != Some("failure") {
        return false;
    }
    if event.category == EventCategory::Git {
        return true;
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
        "git push",
        "git commit",
        "git merge",
        "terraform",
        "fly deploy",
        "vercel",
    ];
    SIGNAL.iter().any(|s| cmd.contains(s))
}

pub fn is_high_signal(event: &CanonicalEvent) -> bool {
    if is_summary_noise(event) {
        return false;
    }
    match event.category {
        EventCategory::Git => true,
        EventCategory::Ide | EventCategory::Build => true,
        EventCategory::Shell => event
            .metadata
            .get("activity_label")
            .and_then(|v| v.as_str())
            .is_some(),
        EventCategory::Os => event.r#type == "process.focus",
        _ => false,
    }
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
}
