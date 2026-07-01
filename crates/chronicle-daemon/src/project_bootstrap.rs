use chronicle_store::Store;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

const DEFAULT_WATCH_DIRS: &[&str] = &["~/Developer", "~/Desktop", "~/Documents"];
const MAX_SCAN_DEPTH: usize = 4;

/// Discover git/cargo repo roots under the given directories.
pub fn discover_repo_paths(watch_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut repos = Vec::new();
    for dir in watch_dirs {
        if dir.is_dir() {
            scan_tree_collect(dir, 0, &mut repos);
        }
    }
    repos
}

/// Discover git/cargo repos on disk (no DB access — safe to run on a blocking thread).
pub fn discover_repos(watch_dirs: &[PathBuf]) -> Vec<(String, String)> {
    discover_repo_paths(watch_dirs)
        .into_iter()
        .filter_map(|path| {
            path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .map(|name| (name, path.to_string_lossy().to_string()))
        })
        .collect()
}

/// Fast path: prune junk rows and upsert from recent events only.
pub fn bootstrap_projects_light(store: &Store) {
    let removed = store.prune_non_repo_projects().unwrap_or(0);
    if removed > 0 {
        info!("pruned {removed} non-repo projects");
    }

    let count = bootstrap_from_recent_events(store);
    if count > 0 {
        info!("updated {count} projects from recent events");
    }
}

/// Full scan when the projects table is empty (runs on a blocking thread).
pub fn apply_discovered_repos(store: &Store, repos: &[(String, String)]) -> usize {
    let mut count = 0usize;
    for (name, path) in repos {
        if store.upsert_project(name, path, None, 0).is_ok() {
            count += 1;
        }
    }
    if count > 0 {
        info!("bootstrapped {count} projects from disk scan");
    }
    count
}

pub fn extra_watch_dirs_from_env() -> Vec<PathBuf> {
    std::env::var("CHRONICLE_WATCH")
        .ok()
        .map(|raw| {
            raw.split(':')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

pub fn default_watch_dirs() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let mut dirs: Vec<PathBuf> = DEFAULT_WATCH_DIRS
        .iter()
        .map(|d| PathBuf::from(d.replace('~', &home)))
        .filter(|p| p.is_dir())
        .collect();
    dirs.extend(volume_developer_dirs());
    dirs
}

fn volume_developer_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let entries = match fs::read_dir("/Volumes") {
        Ok(e) => e,
        Err(_) => return dirs,
    };
    for entry in entries.flatten() {
        let dev = entry.path().join("developer");
        if dev.is_dir() {
            dirs.push(dev);
        }
    }
    dirs
}

fn bootstrap_from_recent_events(store: &Store) -> usize {
    let since = chrono::Utc::now().timestamp_millis() - 30 * 86_400_000;
    let events = match store.query_activity_events(since, None, 500) {
        Ok(events) => events,
        Err(_) => return 0,
    };

    let mut count = 0usize;
    for event in events {
        let path = event
            .metadata
            .get("project_path")
            .and_then(|v| v.as_str())
            .or_else(|| event.metadata.get("cwd").and_then(|v| v.as_str()));

        let Some(path) = path else {
            continue;
        };

        if !is_repo_path(Path::new(path)) {
            continue;
        }

        let name = event
            .project
            .as_deref()
            .or_else(|| Path::new(path).file_name().and_then(|n| n.to_str()))
            .unwrap_or("project");

        if store
            .upsert_project(name, path, None, event.timestamp)
            .is_ok()
        {
            count += 1;
        }
    }
    count
}

fn is_repo_path(path: &Path) -> bool {
    path.is_absolute() && (path.join(".git").exists() || path.join("Cargo.toml").exists())
}

fn scan_tree_collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }

    if is_repo_path(dir) {
        out.push(dir.to_path_buf());
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if should_skip_dir(name) {
            continue;
        }
        scan_tree_collect(&path, depth + 1, out);
    }
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        "node_modules"
            | "target"
            | ".git"
            | "dist"
            | "build"
            | "vendor"
            | "Library"
            | "Caches"
            | ".Trash"
            | ".chronicle"
    ) || name.starts_with('.')
}
