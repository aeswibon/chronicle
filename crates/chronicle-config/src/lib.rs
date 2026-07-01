use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CollectorsConfig {
    #[serde(default = "default_true")]
    pub window_focus: bool,
    #[serde(default = "default_true")]
    pub filesystem: bool,
    #[serde(default = "default_true")]
    pub git: bool,
    #[serde(default = "default_true")]
    pub shell: bool,
}

impl Default for CollectorsConfig {
    fn default() -> Self {
        Self {
            window_focus: true,
            filesystem: true,
            git: true,
            shell: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ChronicleConfig {
    #[serde(default)]
    pub watch_dirs: Vec<String>,
    #[serde(default)]
    pub collectors: CollectorsConfig,
}

pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".chronicle/config.toml")
}

pub fn load() -> ChronicleConfig {
    let path = config_path();
    if !path.exists() {
        return ChronicleConfig::default();
    }
    let raw = fs::read_to_string(&path).unwrap_or_default();
    toml::from_str(&raw).unwrap_or_default()
}

pub fn save(config: &ChronicleConfig) -> anyhow::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = toml::to_string_pretty(config)?;
    fs::write(path, raw)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collectors_default_enabled() {
        let cfg: ChronicleConfig = toml::from_str("watch_dirs = []\n").unwrap();
        assert!(cfg.collectors.window_focus);
        assert!(cfg.collectors.shell);
    }

    #[test]
    fn collectors_partial_toml() {
        let raw = r#"
watch_dirs = ["/tmp"]

[collectors]
shell = false
git = true
"#;
        let cfg: ChronicleConfig = toml::from_str(raw).unwrap();
        assert!(!cfg.collectors.shell);
        assert!(cfg.collectors.git);
        assert!(cfg.collectors.window_focus);
    }
}
