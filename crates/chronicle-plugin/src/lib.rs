use chronicle_core::CanonicalEvent;
use std::collections::HashMap;

pub type PluginResult<T> = Result<T, PluginError>;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("plugin init failed: {0}")]
    InitFailed(String),
    #[error("plugin error: {0}")]
    Runtime(String),
}

pub struct PluginContext {
    pub config: HashMap<String, serde_json::Value>,
    pub data_dir: std::path::PathBuf,
    pub emit: Box<dyn Fn(CanonicalEvent) -> PluginResult<()> + Send>,
}

pub trait ChroniclePlugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn initialize(&mut self, ctx: PluginContext) -> PluginResult<()>;
    fn shutdown(&mut self) -> PluginResult<()>;
}
