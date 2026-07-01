use crate::focus_context::FocusContext;
use std::sync::Arc;
pub mod filesystem;
pub mod git;
pub mod macos_focus;
pub mod macos_window;
pub mod shell;
pub mod window_focus;

use chronicle_core::CanonicalEvent;
use tokio::sync::mpsc;

pub enum Collector {
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    WindowFocus(window_focus::WindowFocusCollector),
    Filesystem(filesystem::FilesystemCollector),
    Git(git::GitCollector),
    Shell(shell::ShellHookCollector),
}

impl Collector {
    pub async fn run(self, tx: mpsc::Sender<CanonicalEvent>, focus: Arc<FocusContext>) {
        match self {
            Collector::WindowFocus(c) => c.run(tx, focus).await,
            Collector::Filesystem(c) => c.run(tx).await,
            Collector::Git(c) => c.run(tx).await,
            Collector::Shell(c) => c.run(tx).await,
        }
    }
}
