use chronicle_core::{CanonicalEvent, EventCategory};
use notify::event::{EventKind, ModifyKind};
use notify::{RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc as tokio_mpsc;
use tracing::{debug, error, info, warn};

use crate::project;

const FS_DEBOUNCE: Duration = Duration::from_secs(3);

const IGNORED_DIR_NAMES: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    "dist",
    "build",
    "__pycache__",
    ".next",
    ".svelte-kit",
    "vendor",
    "DerivedData",
    ".cache",
    "coverage",
    ".turbo",
    ".pnpm-store",
    "Pods",
    ".gradle",
    "tmp",
    ".Trash",
    ".chronicle",
    "Library",
    "Caches",
    "Logs",
];

const IGNORED_FILE_NAMES: &[&str] = &[
    ".DS_Store",
    "Thumbs.db",
    ".gitignore",
    "Cargo.lock",
    "bun.lock",
];

const IGNORED_EXTENSIONS: &[&str] = &[
    "o", "pyc", "pyo", "class", "swp", "swo", "map", "lock", "log", "tmp", "temp",
];

const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "kt", "swift", "c", "cpp", "h", "hpp",
    "cs", "rb", "php", "sql", "md", "toml", "yaml", "yml", "json", "svelte", "vue", "sh", "fish",
    "zsh",
];

pub struct FilesystemCollector {
    watch_dirs: Vec<PathBuf>,
}

impl FilesystemCollector {
    pub fn new(dirs: Option<Vec<PathBuf>>) -> Self {
        let watch_dirs = dirs.unwrap_or_else(|| crate::watch_dirs::resolve_watch_dirs(&[]));
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
            let mut recent: HashMap<String, Instant> = HashMap::new();

            while let Ok(res) = std_rx.recv() {
                let event = match res {
                    Ok(e) => e,
                    Err(e) => {
                        debug!("fs watch error: {e}");
                        continue;
                    }
                };

                let event_type = match filesystem_event_type(&event.kind) {
                    Some(t) => t,
                    None => continue,
                };

                let paths: Vec<&Path> = if matches!(event.kind, EventKind::Modify(ModifyKind::Name(_)))
                    && event.paths.len() == 2
                {
                    vec![event.paths[1].as_path()]
                } else {
                    event.paths.iter().map(|p| p.as_path()).collect()
                };

                for path in paths {
                    if should_ignore(path) {
                        continue;
                    }

                    let key = format!("{event_type}:{}", path.to_string_lossy());
                    let now = Instant::now();
                    if let Some(prev) = recent.get(&key) {
                        if now.duration_since(*prev) < FS_DEBOUNCE {
                            continue;
                        }
                    }
                    recent.insert(key, now);
                    recent.retain(|_, t| now.duration_since(*t) < FS_DEBOUNCE * 2);

                    let source = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "filesystem".into());

                    let mut e = CanonicalEvent::new(&source, EventCategory::Filesystem, event_type);
                    if let Some((project, root)) = project::detect_project(path) {
                        e = e.with_project(&project);
                        let meta = e.metadata.as_object_mut().unwrap();
                        meta.insert("project_path".into(), root.to_string_lossy().into());
                    }

                    let meta = e.metadata.as_object_mut().unwrap();
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        meta.insert("extension".into(), ext.into());
                    }
                    meta.insert("path".into(), path.to_string_lossy().into());
                    if event_type == "file.moved" && event.paths.len() == 2 {
                        meta.insert(
                            "previous_path".into(),
                            event.paths[0].to_string_lossy().into(),
                        );
                    }

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

fn filesystem_event_type(kind: &EventKind) -> Option<&'static str> {
    match kind {
        EventKind::Create(_) => Some("file.created"),
        EventKind::Remove(_) => Some("file.deleted"),
        EventKind::Modify(ModifyKind::Name(_)) => Some("file.moved"),
        _ => None,
    }
}

fn should_ignore(path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if IGNORED_FILE_NAMES.contains(&name) {
            return true;
        }
    }

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if IGNORED_EXTENSIONS.contains(&ext) {
            return true;
        }
        if !SOURCE_EXTENSIONS.contains(&ext) {
            return true;
        }
    } else if path.is_file() {
        return true;
    }

    for comp in path.components() {
        let s = comp.as_os_str().to_string_lossy();
        if IGNORED_DIR_NAMES.iter().any(|d| *d == s.as_ref()) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{EventKind, ModifyKind, RenameMode};

    #[test]
    fn ignores_non_source_extensions() {
        assert!(should_ignore(Path::new("/Volumes/Seagate/developer/photo.png")));
    }

    #[test]
    fn still_ignores_noise_extensions() {
        assert!(should_ignore(Path::new("/Volumes/Seagate/developer/app.log")));
    }

    #[test]
    fn allows_source_extensions() {
        assert!(!should_ignore(Path::new("/Volumes/Seagate/developer/main.rs")));
    }

    #[test]
    fn maps_rename_to_file_moved() {
        let kind = EventKind::Modify(ModifyKind::Name(RenameMode::Both));
        assert_eq!(filesystem_event_type(&kind), Some("file.moved"));
    }
}
