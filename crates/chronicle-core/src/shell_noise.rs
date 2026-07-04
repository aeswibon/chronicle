//! Shared shell-command noise filter for record gates and summary rollups.

/// True when a shell command is navigation, plumbing, or otherwise not worth recording/summarizing.
pub fn is_shell_noise_cmd(cmd: &str) -> bool {
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

    #[test]
    fn treats_cd_as_noise() {
        assert!(is_shell_noise_cmd("cd ~/Developer"));
    }

    #[test]
    fn keeps_cargo_test() {
        assert!(!is_shell_noise_cmd("cargo test -p chronicle-daemon"));
    }
}
