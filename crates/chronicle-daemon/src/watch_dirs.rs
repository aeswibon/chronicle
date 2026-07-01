use crate::project_bootstrap::{default_watch_dirs, extra_watch_dirs_from_env};
use chronicle_config as config;
use std::path::PathBuf;

pub fn expand_path(raw: &str) -> PathBuf {
    PathBuf::from(shellexpand::tilde(raw).to_string())
}

/// Merge CLI flags, saved config, defaults, and CHRONICLE_WATCH env.
pub fn resolve_watch_dirs(cli_watch: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = if !cli_watch.is_empty() {
        cli_watch.to_vec()
    } else {
        let saved = config::load();
        if saved.watch_dirs.is_empty() {
            default_watch_dirs()
        } else {
            saved.watch_dirs.iter().map(|d| expand_path(d)).collect()
        }
    };

    dirs.extend(extra_watch_dirs_from_env());
    dedupe_existing_dirs(dirs)
}

pub fn dedupe_existing_dirs(mut dirs: Vec<PathBuf>) -> Vec<PathBuf> {
    dirs.retain(|p| p.is_dir());
    dirs.sort();
    dirs.dedup();
    dirs
}

pub fn watch_dirs_for_plist(cli_watch: &[String]) -> Vec<PathBuf> {
    let cli: Vec<PathBuf> = cli_watch.iter().map(|d| expand_path(d)).collect();
    resolve_watch_dirs(&cli)
}

pub fn format_env_watch(dirs: &[PathBuf]) -> String {
    dirs.iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn dedupe_sorts_paths() {
        let a = PathBuf::from("/tmp");
        let b = PathBuf::from("/tmp");
        let out = dedupe_existing_dirs(vec![b, a]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], Path::new("/tmp"));
    }
}
