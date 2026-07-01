use chronicle_core::{CanonicalEvent, EventCategory};
use notify::{RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;
use tokio::sync::mpsc as tokio_mpsc;
use tracing::{debug, error, info, warn};

pub struct GitCollector {
    watch_dirs: Vec<PathBuf>,
}

impl GitCollector {
    pub fn new(watch_dirs: Vec<PathBuf>) -> Self {
        Self { watch_dirs }
    }

    fn discover_watch_paths(&self) -> Vec<PathBuf> {
        let roots = if self.watch_dirs.is_empty() {
            crate::watch_dirs::resolve_watch_dirs(&[])
        } else {
            self.watch_dirs.clone()
        };
        let repos = crate::project_bootstrap::discover_repo_paths(&roots);
        let mut paths = Vec::new();
        for repo in repos {
            let logs = repo.join(".git").join("logs");
            if logs.is_dir() {
                paths.push(logs);
            }
            let head = repo.join(".git").join("HEAD");
            if head.is_file() {
                paths.push(head);
            }
        }
        paths
    }

    pub async fn run(self, tx: tokio_mpsc::Sender<CanonicalEvent>) {
        let initial = self.discover_watch_paths();
        if initial.is_empty() {
            info!("git collector: no git repos found in watch dirs, skipping");
            return;
        }

        info!(
            "git collector watching {} paths across repos",
            initial.len()
        );

        let (rescan_tx, rescan_rx) = mpsc::channel::<Vec<PathBuf>>();
        let watch_dirs = self.watch_dirs.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(120)).await;
                let roots = if watch_dirs.is_empty() {
                    crate::watch_dirs::resolve_watch_dirs(&[])
                } else {
                    watch_dirs.clone()
                };
                let repos = crate::project_bootstrap::discover_repo_paths(&roots);
                let mut paths = Vec::new();
                for repo in repos {
                    let logs = repo.join(".git").join("logs");
                    if logs.is_dir() {
                        paths.push(logs);
                    }
                    let head = repo.join(".git").join("HEAD");
                    if head.is_file() {
                        paths.push(head);
                    }
                }
                if rescan_tx.send(paths).is_err() {
                    break;
                }
            }
        });

        let tx_clone = tx;
        tokio::task::spawn_blocking(move || {
            let (std_tx, std_rx) = mpsc::channel::<notify::Result<notify::Event>>();
            let mut watcher =
                match notify::RecommendedWatcher::new(std_tx, notify::Config::default()) {
                    Ok(w) => w,
                    Err(e) => {
                        error!("failed to create git watcher: {e}");
                        return;
                    }
                };

            let mut watched: HashSet<String> = HashSet::new();
            let mut last_seen: HashMap<String, String> = HashMap::new();

            let mut add_paths = |paths: &[PathBuf]| {
                for path in paths {
                    let key = path.to_string_lossy().to_string();
                    if watched.contains(&key) {
                        continue;
                    }
                    let mode = if path.is_dir() {
                        RecursiveMode::Recursive
                    } else {
                        RecursiveMode::NonRecursive
                    };
                    if let Err(e) = watcher.watch(path, mode) {
                        warn!("failed to watch {:?}: {e}", path);
                        continue;
                    }
                    watched.insert(key);
                    debug!("git collector now watching {:?}", path);
                }
            };

            add_paths(&initial);

            loop {
                while let Ok(paths) = rescan_rx.try_recv() {
                    add_paths(&paths);
                }

                match std_rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(Ok(event)) => {
                        for path in &event.paths {
                            let key = path.to_string_lossy().to_string();
                            let fingerprint = file_fingerprint(path);
                            if fingerprint.is_empty() {
                                continue;
                            }
                            if last_seen.get(&key).map(String::as_str) == Some(fingerprint.as_str())
                            {
                                continue;
                            }
                            last_seen.insert(key, fingerprint);

                            if let Some(ev) = parse_git_change(path) {
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
                    Ok(Err(e)) => debug!("git watch error: {e}"),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        });
    }
}

fn file_fingerprint(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn parse_git_change(path: &Path) -> Option<CanonicalEvent> {
    if path.file_name()?.to_str()? == "HEAD" && path.parent()?.ends_with(".git") {
        return parse_head_pointer(path);
    }

    if !path.to_string_lossy().contains("/.git/logs/") {
        return None;
    }

    let content = std::fs::read_to_string(path).ok()?;
    let last_line = content.lines().last()?.trim();
    if last_line.is_empty() {
        return None;
    }
    let parts: Vec<&str> = last_line.split('\t').collect();
    if parts.len() < 2 {
        return None;
    }

    let message = parts[1].trim();
    let message_lower = message.to_lowercase();
    let project = detect_project_from_path(path)?;
    let rel = path
        .to_string_lossy()
        .split("/.git/logs/")
        .nth(1)
        .unwrap_or("")
        .to_string();

    let git_type = if rel.contains("refs/remotes") {
        if message_lower.contains("push") || message_lower.contains("update") {
            "push.completed"
        } else {
            return None;
        }
    } else if message_lower.starts_with("commit") {
        "commit.created"
    } else if message_lower.starts_with("merge") {
        "merge.completed"
    } else if message_lower.starts_with("rebase") {
        "rebase.completed"
    } else if message_lower.contains("push") {
        "push.completed"
    } else if message_lower.contains("checkout") || message_lower.contains("switch to") {
        "branch.checkout"
    } else {
        return None;
    };

    let mut event = CanonicalEvent::new("git", EventCategory::Git, git_type).with_project(&project);
    let meta = event.metadata.as_object_mut().unwrap();
    meta.insert("reflog".into(), message.into());
    meta.insert("ref".into(), rel.into());
    if let Some(root) = project_path_for_git(path) {
        meta.insert("project_path".into(), root.into());
    }
    Some(event)
}

fn parse_head_pointer(path: &Path) -> Option<CanonicalEvent> {
    let content = std::fs::read_to_string(path).ok()?;
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
