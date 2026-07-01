use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ChronicleConfig {
    #[serde(default)]
    pub watch_dirs: Vec<String>,
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
