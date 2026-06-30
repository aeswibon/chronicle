use chronicle_core::{CanonicalEvent, EventCategory};

/// Returns false for events that should not be stored or broadcast.
pub fn should_record(event: &CanonicalEvent) -> bool {
    match event.category {
        EventCategory::Os => should_record_focus(event),
        EventCategory::Shell => should_record_shell(event),
        EventCategory::Git => should_record_git(event),
        EventCategory::Filesystem => should_record_filesystem(event),
        _ => true,
    }
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
    // file.modified is extremely noisy; only keep create/delete.
    matches!(event.r#type.as_str(), "file.created" | "file.deleted")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronicle_core::CanonicalEvent;

    #[test]
    fn skips_chronicle_focus() {
        let mut event = CanonicalEvent::new("chronicle-ui", EventCategory::Os, "process.focus");
        event
            .metadata
            .as_object_mut()
            .unwrap()
            .insert("app_name".into(), "chronicle-ui".into());
        assert!(!should_record(&event));
    }

    #[test]
    fn skips_shell_cd() {
        let mut event = CanonicalEvent::new("cd", EventCategory::Shell, "command.completed");
        event
            .metadata
            .as_object_mut()
            .unwrap()
            .insert("command".into(), "cd /tmp".into());
        assert!(!should_record(&event));
    }

    #[test]
    fn skips_file_modified() {
        let event = CanonicalEvent::new("foo.rs", EventCategory::Filesystem, "file.modified");
        assert!(!should_record(&event));
    }

    #[test]
    fn keeps_git_commit() {
        let event = CanonicalEvent::new("git", EventCategory::Git, "commit.created");
        assert!(should_record(&event));
    }
}
