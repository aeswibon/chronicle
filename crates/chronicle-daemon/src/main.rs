mod cli;
mod collectors;
mod daemon;
mod event_filter;
mod hook_install;
mod project;
mod project_bootstrap;
mod singleton;
mod span_processor;

use clap::Parser;
use cli::{Cli, Commands};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start {
            socket,
            store,
            watch,
        } => {
            let daemon = daemon::Daemon::new(socket, store, watch);
            daemon.run().await
        }
        Commands::Stop { socket } => stop_daemon(&socket).await,
        Commands::Status { socket } => check_status(&socket).await,
        Commands::Install => install_launchd().await,
        Commands::Uninstall => uninstall_launchd().await,
        Commands::Hook { shell } => hook_install::install(shell.as_deref()),
        Commands::HookPrint { shell } => hook_install::print_hook(&shell),
    }
}

async fn stop_daemon(socket: &str) -> anyhow::Result<()> {
    let _client = chronicle_ipc::Client::connect(socket)
        .await
        .map_err(|e| anyhow::anyhow!("daemon not running: {e}"))?;
    println!("Stop signal sent (placeholder)");
    Ok(())
}

async fn check_status(socket: &str) -> anyhow::Result<()> {
    let mut client = chronicle_ipc::Client::connect(socket)
        .await
        .map_err(|e| anyhow::anyhow!("daemon not running: {e}"))?;
    let resp = client
        .request(chronicle_ipc::DaemonRequest::GetStatus)
        .await
        .map_err(|e| anyhow::anyhow!("request failed: {e}"))?;
    match resp {
        chronicle_ipc::DaemonResponse::Status {
            uptime_secs,
            events_count,
            version,
        } => {
            let hours = uptime_secs / 3600;
            let mins = (uptime_secs % 3600) / 60;
            println!("Chronicle v{version}");
            println!("Uptime: {hours}h {mins}m");
            println!("Events recorded: {events_count}");
        }
        _ => println!("Unexpected response"),
    }
    Ok(())
}

async fn install_launchd() -> anyhow::Result<()> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    let plist_path = home.join("Library/LaunchAgents/com.chronicle.daemon.plist");
    let daemon_path = std::env::current_exe()?;
    let store_path = home.join(".chronicle/chronicle.db");
    let log_dir = home.join("Library/Logs");

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.chronicle.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>{daemon}</string>
        <string>start</string>
        <string>--socket</string>
        <string>/tmp/chronicle.sock</string>
        <string>--store</string>
        <string>{store}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{log}/chronicle.log</string>
    <key>StandardErrorPath</key>
    <string>{log}/chronicle.err</string>
</dict>
</plist>"#,
        daemon = daemon_path.display(),
        store = store_path.display(),
        log = log_dir.display(),
    );

    if let Some(parent) = plist_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&plist_path, &plist)?;
    println!("Installed launchd plist at {}", plist_path.display());
    println!("Run: launchctl load {}", plist_path.display());
    Ok(())
}

async fn uninstall_launchd() -> anyhow::Result<()> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    let plist_path = home.join("Library/LaunchAgents/com.chronicle.daemon.plist");

    if plist_path.exists() {
        println!("Run: launchctl unload {}", plist_path.display());
        std::fs::remove_file(&plist_path)?;
        println!("Removed {}", plist_path.display());
    } else {
        println!("No plist found at {}", plist_path.display());
    }
    Ok(())
}
