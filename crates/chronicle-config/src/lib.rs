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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrivacyConfig {
    /// Browser domains to record when non-empty (e.g. github.com).
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default = "default_true")]
    pub strip_query_params: bool,
    /// Delete events/spans older than this many days; None keeps all data.
    #[serde(default)]
    pub retention_days: Option<u32>,
    #[serde(default = "default_true")]
    pub redact_shell_secrets: bool,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            allowed_domains: Vec::new(),
            strip_query_params: true,
            retention_days: None,
            redact_shell_secrets: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ollama_base")]
    pub base_url: String,
    #[serde(default = "default_ai_model")]
    pub model: String,
    /// Env var name holding API key (optional; Ollama needs none).
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default = "default_ai_timeout")]
    pub timeout_secs: u64,
}

fn default_ollama_base() -> String {
    "http://127.0.0.1:11434".into()
}

fn default_ai_model() -> String {
    "smallthinker".into()
}

fn default_ai_timeout() -> u64 {
    60
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: default_ollama_base(),
            model: default_ai_model(),
            api_key_env: None,
            timeout_secs: default_ai_timeout(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SummariesConfig {
    /// Generate today's rollup automatically when the daemon is running.
    #[serde(default = "default_true")]
    pub auto_daily: bool,
    /// Local hour (0–23) after which auto-summarize runs if today has no rollup yet.
    #[serde(default = "default_auto_daily_hour")]
    pub auto_daily_hour_local: u8,
}

fn default_auto_daily_hour() -> u8 {
    21
}

impl Default for SummariesConfig {
    fn default() -> Self {
        Self {
            auto_daily: true,
            auto_daily_hour_local: default_auto_daily_hour(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ChronicleConfig {
    #[serde(default)]
    pub watch_dirs: Vec<String>,
    #[serde(default)]
    pub collectors: CollectorsConfig,
    #[serde(default)]
    pub privacy: PrivacyConfig,
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub summaries: SummariesConfig,
}

pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".chronicle/config.toml")
}

pub fn default_socket_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".chronicle/chronicle.sock")
}

pub fn default_store_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".chronicle/chronicle.db")
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

    #[test]
    fn privacy_defaults() {
        let cfg: ChronicleConfig = toml::from_str("watch_dirs = []\n").unwrap();
        assert!(cfg.privacy.strip_query_params);
        assert!(cfg.privacy.redact_shell_secrets);
        assert!(cfg.privacy.allowed_domains.is_empty());
    }
}
