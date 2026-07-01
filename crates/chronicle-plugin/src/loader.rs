use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub entry: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRecord {
    pub manifest: PluginManifest,
    pub path: PathBuf,
}

/// Scan `~/.chronicle/plugins/*/` for `chronicle-plugin.toml` or `plugin.json`.
pub fn discover_plugins() -> Vec<PluginRecord> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let root = home.join(".chronicle/plugins");
    if !root.is_dir() {
        return Vec::new();
    }

    let mut found = Vec::new();
    let entries = std::fs::read_dir(&root).ok();
    let Some(entries) = entries else {
        return found;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(record) = load_manifest(&path) {
            found.push(record);
        }
    }
    found.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    found
}

fn load_manifest(dir: &Path) -> Option<PluginRecord> {
    for file in ["chronicle-plugin.toml", "plugin.json"] {
        let path = dir.join(file);
        if !path.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&path).ok()?;
        let manifest: PluginManifest = if file.ends_with(".toml") {
            toml::from_str(&raw).ok()?
        } else {
            serde_json::from_str(&raw).ok()?
        };
        return Some(PluginRecord {
            manifest,
            path: dir.to_path_buf(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_manifest() {
        let dir =
            std::env::temp_dir().join(format!("chronicle-plugin-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("plugin.json"),
            r#"{"name":"demo","version":"0.1.0"}"#,
        )
        .unwrap();
        let record = load_manifest(&dir).unwrap();
        assert_eq!(record.manifest.name, "demo");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
