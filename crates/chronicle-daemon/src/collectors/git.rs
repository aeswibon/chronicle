use chronicle_core::{CanonicalEvent, EventCategory};
use notify::{RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use tokio::sync::mpsc as tokio_mpsc;
use tracing::{debug, error, info, warn};

pub struct GitCollector {
    watch_dirs: Vec<PathBuf>,
}

impl GitCollector {
    pub fn new(watch_dirs: Vec<PathBuf>) -> Self {
        Self { watch_dirs }
    }

    fn find_git_logs(&self) -> Vec<PathBuf> {
        let roots = if self.watch_dirs.is_empty() {
            crate::watch_dirs::resolve_watch_dirs(&[])
        } else {
            self.watch_dirs.clone()
        };
        let repos = crate::project_bootstrap::discover_repo_paths(&roots);
        let mut logs = Vec::new();
        for repo in repos {
            let git_dir = repo.join(".git");
            let reflog = git_dir.join("logs").join("HEAD");
            if reflog.exists() {
                logs.push(reflog);
            }
            let head = git_dir.join("HEAD");
            if head.exists() {
                logs.push(head);
            }
        }
        logs
    }

    pub async fn run(self, tx: tokio_mpsc::Sender<CanonicalEvent>) {
        let git_logs = self.find_git_logs();
        if git_logs.is_empty() {
            info!("git collector: no git repos found in watch dirs, skipping");
            return;
        }

        info!("git collector watching {} reflogs", git_logs.len());

        let (std_tx, std_rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = match notify::RecommendedWatcher::new(std_tx, notify::Config::default()) {
            Ok(w) => w,
            Err(e) => {
                error!("failed to create git watcher: {e}");
                return;
            }
        };

        for log in &git_logs {
            if let Err(e) = watcher.watch(log, RecursiveMode::NonRecursive) {
                warn!("failed to watch {:?}: {e}", log);
            }
        }

        let tx_clone = tx;
        tokio::task::spawn_blocking(move || {
            let mut last_seen: HashMap<String, String> = HashMap::new();

            while let Ok(res) = std_rx.recv() {
                let event = match res {
                    Ok(e) => e,
                    Err(e) => {
                        debug!("git watch error: {e}");
                        continue;
                    }
                };

                for path in &event.paths {
                    let key = path.to_string_lossy().to_string();
                    let fingerprint = file_fingerprint(path);
                    if fingerprint.is_empty() {
                        continue;
                    }
                    if last_seen.get(&key).map(String::as_str) == Some(fingerprint.as_str()) {
                        continue;
                    }
                    last_seen.insert(key, fingerprint);

                    if let Some(ev) = parse_reflog_change(path) {
                        debug!(
                            "git event: {} in {}",
                            ev.r#type,
                            ev.project.as_deref().unwrap_or("?")
                        );
                        if tx_clone.blocking_send(ev).is_err() {
                            warn!("git: receiver dropped");
                            return;
                        }
                    }
                }
            }
        });
    }
}

fn file_fingerprint(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn parse_reflog_change(path: &Path) -> Option<CanonicalEvent> {
    let file_name = path.file_name()?.to_str()?;

    if file_name == "HEAD" {
        let content = std::fs::read_to_string(path).ok()?;
        let branch = content.trim().strip_prefix("ref: refs/heads/")?;
        let project = detect_project_from_path(path)?;
        let project_path = project_path_for_git(path)?;
        let mut event = CanonicalEvent::new("git", EventCategory::Git, "branch.checkout")
            .with_project(&project);
        let meta = event.metadata.as_object_mut().unwrap();
        meta.insert("branch".into(), branch.into());
        meta.insert("project_path".into(), project_path.into());
        return Some(event);
    }

    let content = std::fs::read_to_string(path).ok()?;
    let last_line = content.lines().last()?;
    let parts: Vec<&str> = last_line.split('\t').collect();
    if parts.len() < 2 {
        return None;
    }

    let message = parts[1].trim();
    let project = detect_project_from_path(path)?;

    let git_type = if message.starts_with("commit") {
        "commit.created"
    } else if message.starts_with("merge") {
        "merge.completed"
    } else if message.starts_with("rebase") {
        "rebase.completed"
    } else if message.contains("checkout") {
        "branch.checkout"
    } else {
        return None;
    };

    let mut event = CanonicalEvent::new("git", EventCategory::Git, git_type).with_project(&project);
    let meta = event.metadata.as_object_mut().unwrap();
    meta.insert("reflog".into(), message.into());
    if let Some(root) = project_path_for_git(path) {
        meta.insert("project_path".into(), root.into());
    }
    Some(event)
}

fn project_path_for_git(path: &Path) -> Option<String> {
    for ancestor in path.ancestors() {
        if ancestor.join(".git").exists() {
            return Some(ancestor.to_string_lossy().to_string());
        }
    }
    None
}

fn detect_project_from_path(path: &Path) -> Option<String> {
    for ancestor in path.ancestors() {
        if ancestor.join(".git").exists() {
            return ancestor
                .file_name()
                .map(|n| n.to_string_lossy().to_string());
        }
    }
    None
}
