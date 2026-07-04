use chronicle_core::{CanonicalEvent, EventCategory};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc as tokio_mpsc;
use tracing::{debug, info, warn};

const POLL_INTERVAL: Duration = Duration::from_secs(20);
const RESCAN_INTERVAL: Duration = Duration::from_secs(120);
/// Limited backfill for HEAD/main only when cursors already exist (upgrade path).
const BACKFILL_WINDOW_MS: i64 = 12 * 3_600_000;
const BACKFILL_MAX_LINES: usize = 40;

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct PersistedGitCursors {
    line_counts: HashMap<String, usize>,
    head_content: HashMap<String, String>,
    #[serde(default)]
    backfill_done: HashSet<String>,
}

fn cursors_path() -> PathBuf {
    chronicle_config::default_store_path()
        .parent()
        .map(|p| p.join("git_cursors.json"))
        .unwrap_or_else(|| PathBuf::from("/tmp/chronicle_git_cursors.json"))
}

struct GitScanState {
    line_counts: HashMap<String, usize>,
    head_content: HashMap<String, String>,
    backfill_done: HashSet<String>,
    dirty: bool,
    logged_repos: HashSet<String>,
    /// First daemon run: seed cursors only — do not ingest historical reflogs.
    bootstrap: bool,
}

impl GitScanState {
    fn load() -> Self {
        let path = cursors_path();
        let file_existed = path.exists();
        let persisted: PersistedGitCursors = std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        Self {
            line_counts: persisted.line_counts,
            head_content: persisted.head_content,
            backfill_done: persisted.backfill_done,
            dirty: false,
            logged_repos: HashSet::new(),
            bootstrap: !file_existed,
        }
    }

    fn save_if_dirty(&mut self) {
        if !self.dirty {
            return;
        }
        let path = cursors_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let persisted = PersistedGitCursors {
            line_counts: self.line_counts.clone(),
            head_content: self.head_content.clone(),
            backfill_done: self.backfill_done.clone(),
        };
        if let Ok(raw) = serde_json::to_string(&persisted) {
            let _ = std::fs::write(path, raw);
            self.dirty = false;
        }
    }
}

impl Default for GitScanState {
    fn default() -> Self {
        Self {
            line_counts: HashMap::new(),
            head_content: HashMap::new(),
            backfill_done: HashSet::new(),
            dirty: false,
            logged_repos: HashSet::new(),
            bootstrap: false,
        }
    }
}

pub struct GitCollector {
    watch_dirs: Vec<PathBuf>,
}

impl GitCollector {
    pub fn new(watch_dirs: Vec<PathBuf>) -> Self {
        Self { watch_dirs }
    }

    pub async fn run(self, tx: tokio_mpsc::Sender<CanonicalEvent>) {
        let watch_dirs = self.watch_dirs.clone();
        tokio::task::spawn_blocking(move || {
            let mut state = GitScanState::load();
            let mut roots = if watch_dirs.is_empty() {
                crate::watch_dirs::resolve_watch_dirs(&[])
            } else {
                watch_dirs.clone()
            };
            let mut last_rescan = std::time::Instant::now();

            loop {
                if last_rescan.elapsed() >= RESCAN_INTERVAL {
                    roots = if watch_dirs.is_empty() {
                        crate::watch_dirs::resolve_watch_dirs(&[])
                    } else {
                        watch_dirs.clone()
                    };
                    last_rescan = std::time::Instant::now();
                }

                let repos = crate::project_bootstrap::discover_repo_paths(&roots);
                if repos.is_empty() {
                    debug!("git collector: no repos under watch dirs");
                } else {
                    let emitted = scan_repos(&repos, &mut state, &tx);
                    if emitted > 0 {
                        debug!("git collector: emitted {emitted} events");
                    }
                    state.save_if_dirty();
                }

                std::thread::sleep(POLL_INTERVAL);
            }
        });
    }
}

fn scan_repos(
    repos: &[PathBuf],
    state: &mut GitScanState,
    tx: &tokio_mpsc::Sender<CanonicalEvent>,
) -> usize {
    if state.bootstrap {
        bootstrap_cursors(repos, state);
        state.bootstrap = false;
        state.save_if_dirty();
        info!("git collector: initialized cursors (skipping historical backfill on first run)");
        return 0;
    }

    let mut emitted = 0usize;
    for repo in repos {
        let key = repo.to_string_lossy().to_string();
        if state.logged_repos.insert(key.clone()) {
            info!("git collector tracking {:?}", repo);
        }

        let logs_root = repo.join(".git").join("logs");
        if !logs_root.is_dir() {
            continue;
        }

        let mut files = Vec::new();
        collect_reflog_files(&logs_root, &mut files);
        let head = repo.join(".git").join("HEAD");
        if head.is_file() {
            files.push(head);
        }

        for path in files {
            emitted += scan_reflog_file(&path, state, tx);
        }
    }
    emitted
}

fn bootstrap_cursors(repos: &[PathBuf], state: &mut GitScanState) {
    for repo in repos {
        let logs_root = repo.join(".git").join("logs");
        if !logs_root.is_dir() {
            continue;
        }
        let mut files = Vec::new();
        collect_reflog_files(&logs_root, &mut files);
        let head = repo.join(".git").join("HEAD");
        if head.is_file() {
            if let Ok(content) = std::fs::read_to_string(&head) {
                let key = head.to_string_lossy().to_string();
                state.head_content.insert(key, content);
                state.dirty = true;
            }
        }
        for path in files {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let count = content
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .count();
                let key = path.to_string_lossy().to_string();
                state.line_counts.insert(key, count);
                state.dirty = true;
            }
        }
    }
}

fn collect_reflog_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_reflog_files(&path, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}

fn scan_reflog_file(
    path: &Path,
    state: &mut GitScanState,
    tx: &tokio_mpsc::Sender<CanonicalEvent>,
) -> usize {
    if path.file_name().and_then(|n| n.to_str()) == Some("HEAD")
        && path.parent().is_some_and(|p| p.ends_with(".git"))
    {
        return scan_head_pointer(path, state, tx);
    }

    if !path.to_string_lossy().contains("/.git/logs/") {
        return 0;
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let lines: Vec<&str> = content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return 0;
    }

    let key = path.to_string_lossy().to_string();
    let prev_count = state.line_counts.get(&key).copied().unwrap_or(0);
    let rel = path
        .to_string_lossy()
        .split("/.git/logs/")
        .nth(1)
        .unwrap_or("")
        .to_string();

    let now_ms = chrono::Utc::now().timestamp_millis();
    let cutoff = now_ms - BACKFILL_WINDOW_MS;

    let mut emitted = 0usize;
    let mut backfill_budget = BACKFILL_MAX_LINES;
    for (idx, line) in lines.iter().enumerate() {
        let is_new = idx >= prev_count;
        let in_backfill = prev_count > 0
            && !state.backfill_done.contains(&key)
            && backfill_budget > 0
            && is_backfill_candidate(&rel)
            && parse_reflog_timestamp_ms(line).is_some_and(|ts| ts >= cutoff);
        if !is_new && !in_backfill {
            continue;
        }

        if let Some(event) = parse_reflog_line(path, line) {
            if tx.blocking_send(event).is_err() {
                warn!("git: receiver dropped");
                return emitted;
            }
            emitted += 1;
            if in_backfill {
                backfill_budget -= 1;
            }
        }
    }

    if prev_count > 0 && !state.backfill_done.contains(&key) {
        state.backfill_done.insert(key.clone());
        state.dirty = true;
    }
    if prev_count != lines.len() {
        state.line_counts.insert(key, lines.len());
        state.dirty = true;
    }
    emitted
}

fn scan_head_pointer(
    path: &Path,
    state: &mut GitScanState,
    tx: &tokio_mpsc::Sender<CanonicalEvent>,
) -> usize {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let key = path.to_string_lossy().to_string();
    if state
        .head_content
        .get(&key)
        .is_some_and(|prev| prev == &content)
    {
        return 0;
    }

    let emitted = if let Some(event) = parse_head_pointer(path, &content) {
        if tx.blocking_send(event).is_err() {
            warn!("git: receiver dropped");
            return 0;
        }
        1
    } else {
        0
    };

    state.head_content.insert(key, content);
    state.dirty = true;
    emitted
}

fn parse_reflog_line(path: &Path, line: &str) -> Option<CanonicalEvent> {
    let parts: Vec<&str> = line.splitn(2, '\t').collect();
    if parts.len() < 2 {
        return None;
    }

    let message = parts[1].trim();
    if message.is_empty() {
        return None;
    }

    let rel = path
        .to_string_lossy()
        .split("/.git/logs/")
        .nth(1)
        .unwrap_or("")
        .to_string();

    let git_type = classify_git_event(&rel, message)?;
    let project = detect_project_from_path(path)?;
    let timestamp =
        parse_reflog_timestamp_ms(line).unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    let mut event = CanonicalEvent::new("git", EventCategory::Git, git_type).with_project(&project);
    event.timestamp = timestamp;

    let meta = event.metadata.as_object_mut().unwrap();
    let repo_root = repo_root(path);
    enrich_reflog_metadata(meta, line, message, repo_root.as_deref());
    meta.insert("ref".into(), rel.into());
    if let Some(root) = project_path_for_git(path) {
        meta.insert("project_path".into(), root.into());
    }
    Some(event)
}

fn git_commit_shortstat(repo_root: &Path, commit: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args([
            "-C",
            &repo_root.to_string_lossy(),
            "show",
            "--shortstat",
            "-s",
            commit,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.contains("file changed") || trimmed.contains("files changed") {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn enrich_reflog_metadata(
    meta: &mut serde_json::Map<String, serde_json::Value>,
    line: &str,
    message: &str,
    repo_root: Option<&Path>,
) {
    meta.insert("reflog".into(), message.into());
    let header = line.split('\t').next().unwrap_or(line);
    let tokens: Vec<&str> = header.split_whitespace().collect();
    if tokens.len() >= 2 {
        meta.insert("commit_hash".into(), tokens[1].into());
    }
    if tokens.len() >= 3 {
        meta.insert("commit_author".into(), tokens[2].into());
    }
    if let Some(msg) = message.strip_prefix("commit: ") {
        meta.insert("commit_message".into(), msg.trim().into());
    }
    if message.starts_with("commit:") {
        if let Some(hash) = meta.get("commit_hash").and_then(|v| v.as_str()) {
            if let Some(root) = repo_root {
                if let Some(stats) = git_commit_shortstat(root, hash) {
                    meta.insert("diff_stats".into(), stats.into());
                }
            }
        }
    }
}

fn parse_head_pointer(path: &Path, content: &str) -> Option<CanonicalEvent> {
    let branch = content.trim().strip_prefix("ref: refs/heads/")?;
    let project = detect_project_from_path(path)?;
    let project_path = project_path_for_git(path)?;
    let mut event =
        CanonicalEvent::new("git", EventCategory::Git, "branch.checkout").with_project(&project);
    let meta = event.metadata.as_object_mut().unwrap();
    meta.insert("branch".into(), branch.into());
    meta.insert("project_path".into(), project_path.into());
    Some(event)
}

fn is_backfill_candidate(rel: &str) -> bool {
    rel == "HEAD" || rel == "refs/heads/master" || rel == "refs/heads/main"
}

fn classify_git_event(rel: &str, message: &str) -> Option<&'static str> {
    let message_lower = message.to_lowercase();

    if rel.contains("refs/remotes") {
        if message_lower.contains("update by push") || message_lower.contains("push") {
            return Some("push.completed");
        }
        if message_lower.contains("fetch")
            || message_lower.contains("fast-forward")
            || message_lower.contains("pull")
        {
            return Some("pull.completed");
        }
        return None;
    }

    if message_lower.starts_with("commit") || message_lower.contains(": commit") {
        return Some("commit.created");
    }
    if message_lower.starts_with("merge") {
        return Some("merge.completed");
    }
    if message_lower.starts_with("rebase") {
        return Some("rebase.completed");
    }
    if message_lower.contains("pull") {
        return Some("pull.completed");
    }
    if message_lower.starts_with("fetch") {
        return Some("fetch.completed");
    }
    if message_lower.contains("push") {
        return Some("push.completed");
    }
    if message_lower.contains("checkout") || message_lower.contains("switch to") {
        return Some("branch.checkout");
    }
    None
}

/// Reflog header: `old new Name <email> unix_secs +tz\tmessage`
pub fn parse_reflog_timestamp_ms(line: &str) -> Option<i64> {
    let header = line.split('\t').next()?;
    let parts: Vec<&str> = header.split_whitespace().collect();
    for (i, part) in parts.iter().enumerate() {
        if (part.starts_with('+') || part.starts_with('-')) && i > 0 {
            return parts[i - 1].parse::<i64>().ok().map(|s| s * 1000);
        }
    }
    None
}

fn project_path_for_git(path: &Path) -> Option<String> {
    repo_root(path).map(|p| p.to_string_lossy().to_string())
}

fn detect_project_from_path(path: &Path) -> Option<String> {
    repo_root(path).and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
}

fn repo_root(path: &Path) -> Option<PathBuf> {
    for ancestor in path.ancestors() {
        if ancestor.join(".git").exists() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reflog_timestamp_from_commit_line() {
        let line = "acbb3ae2c3e537ac0c57b66f6693ca983123f9ee 3c4d0af0c84530942db52c213c11e5ca58b6c54c Abhiuday <gupta.abhiuday.109@gmail.com> 1782882724 +0530\tcommit: Add AI-enhanced daily summaries.";
        assert_eq!(parse_reflog_timestamp_ms(line), Some(1782882724 * 1000));
    }

    #[test]
    fn classify_push_on_remote_ref() {
        assert_eq!(
            classify_git_event("refs/remotes/origin/master", "update by push"),
            Some("push.completed")
        );
    }

    #[test]
    fn classify_pull_fast_forward_on_head() {
        assert_eq!(
            classify_git_event("HEAD", "pull origin master: Fast-forward"),
            Some("pull.completed")
        );
    }

    #[test]
    fn classify_fetch_on_remote() {
        assert_eq!(
            classify_git_event("refs/remotes/origin/master", "fetch origin: fast-forward"),
            Some("pull.completed")
        );
    }

    #[test]
    fn classify_commit() {
        assert_eq!(
            classify_git_event("HEAD", "commit: fix something"),
            Some("commit.created")
        );
    }
}
