use std::path::{Path, PathBuf};

/// Find the git or cargo project root name and path for a file or directory.
pub fn detect_project(path: &Path) -> Option<(String, PathBuf)> {
    for ancestor in path.ancestors() {
        if ancestor.join(".git").exists() || ancestor.join("Cargo.toml").exists() {
            let name = ancestor.file_name()?.to_string_lossy().to_string();
            return Some((name, ancestor.to_path_buf()));
        }
    }
    None
}

pub fn project_name_from_cwd(cwd: &str) -> Option<String> {
    detect_project(Path::new(cwd)).map(|(name, _)| name)
}

/// Parse IDE window titles like "file.ts — project — Cursor" for repo roots.
pub fn detect_project_from_title(title: &str) -> Option<(String, PathBuf)> {
    for token in title.split(['/', ' ', '—', '–', '-']) {
        if token.is_empty() {
            continue;
        }
        let path = Path::new(token);
        if path.is_absolute() {
            if let Some(found) = detect_project(path) {
                return Some(found);
            }
        }
    }
    None
}

pub fn project_path_from_event(project: &str, metadata: &serde_json::Value) -> Option<String> {
    if let Some(path) = metadata.get("project_path").and_then(|v| v.as_str()) {
        return Some(path.to_string());
    }
    if let Some(path) = metadata.get("path").and_then(|v| v.as_str()) {
        if let Some((name, root)) = detect_project(Path::new(path)) {
            if name == project {
                return Some(root.to_string_lossy().to_string());
            }
        }
    }
    if let Some(cwd) = metadata.get("cwd").and_then(|v| v.as_str()) {
        if let Some((name, root)) = detect_project(Path::new(cwd)) {
            if name == project {
                return Some(root.to_string_lossy().to_string());
            }
        }
    }
    None
}
