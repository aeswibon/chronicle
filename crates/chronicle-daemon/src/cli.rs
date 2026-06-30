use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "chronicle", about = "Developer observability daemon")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the Chronicle daemon
    Start {
        /// Path to the UDS socket
        #[arg(long, default_value = "/tmp/chronicle.sock")]
        socket: String,

        /// Path to the SQLite store
        #[arg(long, default_value = "~/.chronicle/chronicle.db")]
        store: String,

        /// Directories to watch for file changes
        #[arg(long)]
        watch: Vec<String>,
    },

    /// Stop the running daemon
    Stop {
        /// Path to the UDS socket
        #[arg(long, default_value = "/tmp/chronicle.sock")]
        socket: String,
    },

    /// Show daemon status
    Status {
        /// Path to the UDS socket
        #[arg(long, default_value = "/tmp/chronicle.sock")]
        socket: String,
    },

    /// Install launchd plist
    Install,

    /// Uninstall launchd plist
    Uninstall,
}
