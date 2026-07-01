mod cli;
mod collectors;
mod daemon;
mod event_filter;
mod http_ingress;
mod project;
mod project_bootstrap;
mod rule_engine;
mod singleton;
mod span_processor;
mod watch_dirs;

use chronicle_config as config;
use chronicle_hooks as hook_install;

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
        Commands::Install { watch } => install_launchd(&watch).await,
        Commands::Uninstall => uninstall_launchd().await,
        Commands::Hook { shell } => hook_install::install_and_print(shell.as_deref()),
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

async fn install_launchd(cli_watch: &[String]) -> anyhow::Result<()> {
    if !cli_watch.is_empty() {
        let mut cfg = config::load();
        cfg.watch_dirs = cli_watch.to_vec();
        config::save(&cfg)?;
    }

    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    let plist_path = home.join("Library/LaunchAgents/com.chronicle.daemon.plist");
    let daemon_path = std::env::current_exe()?.canonicalize().unwrap_or_else(|_| {
        std::env::current_exe().expect("current_exe")
    });
    let store_path = chronicle_config::default_store_path();
    let socket_path = chronicle_config::default_socket_path();
    let log_dir = home.join("Library/Logs");
    let watch_dirs = watch_dirs::watch_dirs_for_plist(cli_watch);
    let watch_env = watch_dirs::format_env_watch(&watch_dirs);

    let mut program_args = vec![
        format!("<string>{}</string>", daemon_path.display()),
        "<string>start</string>".into(),
        "<string>--socket</string>".into(),
        format!("<string>{}</string>", socket_path.display()),
        "<string>--store</string>".into(),
        format!("<string>{}</string>", store_path.display()),
    ];
    for dir in &watch_dirs {
        program_args.push("<string>--watch</string>".into());
        program_args.push(format!("<string>{}</string>", dir.display()));
    }
    let program_args_xml = program_args.join("\n        ");

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.chronicle.daemon</string>
    <key>ProgramArguments</key>
    <array>
        {program_args_xml}
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>CHRONICLE_WATCH</key>
        <string>{watch_env}</string>
    </dict>
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
        log = log_dir.display(),
    );

    if let Some(parent) = plist_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&plist_path, &plist)?;
    println!("Installed launchd plist at {}", plist_path.display());
    if !watch_dirs.is_empty() {
        println!("Watch directories:");
        for dir in &watch_dirs {
            println!("  {}", dir.display());
        }
    }
    if let Err(e) = activate_launch_agent(&plist_path) {
        println!("Note: auto-start via launchctl failed ({e}).");
        println!(
            "Run manually: launchctl bootstrap gui/$(id -u) {}",
            plist_path.display()
        );
    } else {
        println!("Daemon started (user LaunchAgent, no admin password required).");
    }
    Ok(())
}

fn activate_launch_agent(plist_path: &std::path::Path) -> anyhow::Result<()> {
    let uid = std::process::Command::new("id").arg("-u").output()?;
    let uid = String::from_utf8_lossy(&uid.stdout).trim().to_string();
    let domain = format!("gui/{uid}");
    let service = format!("{domain}/com.chronicle.daemon");
    let plist_str = plist_path.to_string_lossy().to_string();

    let _ = std::process::Command::new("launchctl")
        .args(["bootout", &service])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    let bootstrap = std::process::Command::new("launchctl")
        .args(["bootstrap", &domain, &plist_str])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if !bootstrap.success() {
        anyhow::bail!("launchctl bootstrap exited with {}", bootstrap);
    }

    let kick = std::process::Command::new("launchctl")
        .args(["kickstart", "-k", &service])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if kick.success() {
        Ok(())
    } else {
        anyhow::bail!("launchctl kickstart failed")
    }
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
