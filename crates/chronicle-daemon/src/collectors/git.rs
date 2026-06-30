use chronicle_core::{CanonicalEvent, EventCategory};
use notify::{RecursiveMode, Watcher};
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
        let mut logs = Vec::new();
        for dir in &self.watch_dirs {
            let git_dir = dir.join(".git");
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
            while let Ok(res) = std_rx.recv() {
                let event = match res {
                    Ok(e) => e,
                    Err(e) => {
                        debug!("git watch error: {e}");
                        continue;
                    }
                };

                for path in &event.paths {
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

fn parse_reflog_change(path: &Path) -> Option<CanonicalEvent> {
    let file_name = path.file_name()?.to_str()?;

    if file_name == "HEAD" {
        let content = std::fs::read_to_string(path).ok()?;
        let branch = content.trim().strip_prefix("ref: refs/heads/")?;
        let project = detect_project_from_path(path)?;
        let mut event = CanonicalEvent::new(
            "chronicle-daemon",
            EventCategory::Git,
            "branch.checkout",
        )
        .with_project(&project);
        let meta = event.metadata.as_object_mut().unwrap();
        meta.insert("branch".into(), branch.into());
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
        "git.other"
    };

    let mut event = CanonicalEvent::new("chronicle-daemon", EventCategory::Git, git_type)
        .with_project(&project);
    let meta = event.metadata.as_object_mut().unwrap();
    meta.insert("reflog".into(), message.into());
    Some(event)
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
