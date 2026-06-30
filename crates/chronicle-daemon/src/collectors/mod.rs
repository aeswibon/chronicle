pub mod window_focus;
pub mod filesystem;
pub mod git;
pub mod shell;

use chronicle_core::CanonicalEvent;
use tokio::sync::mpsc;

pub enum Collector {
    WindowFocus(window_focus::WindowFocusCollector),
    Filesystem(filesystem::FilesystemCollector),
    Git(git::GitCollector),
    Shell(shell::ShellHookCollector),
}

impl Collector {
    pub async fn run(self, tx: mpsc::Sender<CanonicalEvent>) {
        match self {
            Collector::WindowFocus(c) => c.run(tx).await,
            Collector::Filesystem(c) => c.run(tx).await,
            Collector::Git(c) => c.run(tx).await,
            Collector::Shell(c) => c.run(tx).await,
        }
    }
}
