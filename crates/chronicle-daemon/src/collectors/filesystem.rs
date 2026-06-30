use chronicle_core::{CanonicalEvent, EventCategory};
use notify::{EventKind, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use tokio::sync::mpsc as tokio_mpsc;
use tracing::{debug, error, info, warn};

const DEFAULT_WATCH_DIRS: &[&str] = &["~/Developer", "~/Desktop", "~/Documents"];

pub struct FilesystemCollector {
    watch_dirs: Vec<PathBuf>,
}

impl FilesystemCollector {
    pub fn new(dirs: Option<Vec<PathBuf>>) -> Self {
        let watch_dirs = dirs.unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            DEFAULT_WATCH_DIRS
                .iter()
                .map(|d| PathBuf::from(d.replace('~', &home)))
                .filter(|p| p.exists())
                .collect()
        });
        Self { watch_dirs }
    }

    pub async fn run(self, tx: tokio_mpsc::Sender<CanonicalEvent>) {
        info!("filesystem collector watching {:?}", self.watch_dirs);

        let (std_tx, std_rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = match notify::RecommendedWatcher::new(std_tx, notify::Config::default()) {
            Ok(w) => w,
            Err(e) => {
                error!("failed to create watcher: {e}");
                return;
            }
        };

        for dir in &self.watch_dirs {
            if let Err(e) = watcher.watch(dir, RecursiveMode::Recursive) {
                warn!("failed to watch {:?}: {e}", dir);
            }
        }

        let tx_clone = tx;
        tokio::task::spawn_blocking(move || {
            while let Ok(res) = std_rx.recv() {
                let event = match res {
                    Ok(e) => e,
                    Err(e) => {
                        debug!("fs watch error: {e}");
                        continue;
                    }
                };

                let event_type = match event.kind {
                    EventKind::Create(_) => "file.created",
                    EventKind::Remove(_) => "file.deleted",
                    _ => "file.modified",
                };

                for path in &event.paths {
                    if should_ignore(path) {
                        continue;
                    }

                    let mut e = CanonicalEvent::new(
                        "chronicle-daemon",
                        EventCategory::Filesystem,
                        event_type,
                    );
                    if let Some(project) = detect_project(path) {
                        e = e.with_project(&project);
                    }

                    let meta = e.metadata.as_object_mut().unwrap();
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        meta.insert("extension".into(), ext.into());
                    }
                    meta.insert("path".into(), path.to_string_lossy().into());

                    debug!("fs event: {} {}", event_type, path.display());

                    if tx_clone.blocking_send(e).is_err() {
                        warn!("filesystem: receiver dropped");
                        return;
                    }
                }
            }
        });
    }
}

fn should_ignore(path: &std::path::Path) -> bool {
    for comp in path.components() {
        let s = comp.as_os_str().to_string_lossy();
        if s.starts_with('.') && s != "." {
            return true;
        }
    }
    path.to_string_lossy().contains("/.git/")
}

fn detect_project(path: &std::path::Path) -> Option<String> {
    for ancestor in path.ancestors() {
        if ancestor.join(".git").exists() || ancestor.join("Cargo.toml").exists() {
            return ancestor
                .file_name()
                .map(|n| n.to_string_lossy().to_string());
        }
    }
    None
}
