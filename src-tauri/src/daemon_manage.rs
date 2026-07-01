//! Locate and start chronicle-daemon without admin rights (user LaunchAgent).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

pub fn default_socket_string() -> String {
    chronicle_config::default_socket_path()
        .to_string_lossy()
        .into_owned()
}

pub fn resolve_daemon_binary() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in ["chronicle-daemon", "binaries/chronicle-daemon"] {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    if let Some(home) = dirs::home_dir() {
        for rel in [
            ".local/bin/chronicle-daemon",
            "bin/chronicle-daemon",
            ".cargo/bin/chronicle-daemon",
        ] {
            let candidate = home.join(rel);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for rel in [
        "../target/debug/chronicle-daemon",
        "../target/release/chronicle-daemon",
        "target/debug/chronicle-daemon",
        "target/release/chronicle-daemon",
    ] {
        let candidate = manifest.join(rel);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

pub fn daemon_reachable(socket: &str) -> bool {
    std::thread::spawn({
        let socket = socket.to_string();
        move || {
            let rt = tokio::runtime::Runtime::new().ok()?;
            rt.block_on(async {
                chronicle_ipc::Client::connect(&socket)
                    .await
                    .ok()?
                    .request(chronicle_ipc::DaemonRequest::GetStatus)
                    .await
                    .ok()
            })
        }
    })
    .join()
    .ok()
    .flatten()
    .is_some()
}

/// Install the user LaunchAgent and start the daemon. No administrator password required.
pub fn ensure_daemon_running() -> Result<(), String> {
    let socket = default_socket_string();
    if daemon_reachable(&socket) {
        return Ok(());
    }

    let binary = resolve_daemon_binary()
        .ok_or_else(|| "Chronicle daemon binary not found. Reinstall the app.".to_string())?;

    // Dev builds: avoid launchd (KeepAlive fights the singleton lock). Spawn once in user space.
    if cfg!(debug_assertions) {
        unload_launch_agent();
        spawn_detached_daemon(&binary, &socket)?;
        if wait_for_daemon(&socket, 12) {
            return Ok(());
        }
        return Err(
            "Chronicle could not start the background service. Build chronicle-daemon (cargo build -p chronicle-daemon)."
                .into(),
        );
    }

    if needs_plist_install(&binary) {
        run_daemon_install(&binary)?;
    }

    if kickstart_launch_agent() && wait_for_daemon(&socket, 8) {
        return Ok(());
    }

    if !is_launch_agent_loaded()
        && activate_launch_agent().is_ok()
        && wait_for_daemon(&socket, 8)
    {
        return Ok(());
    }

    spawn_detached_daemon(&binary, &socket)?;
    if wait_for_daemon(&socket, 12) {
        return Ok(());
    }

    Err("Chronicle could not start the background service. Check Settings → Restart daemon.".into())
}

pub fn restart_daemon() -> Result<(), String> {
    let binary = resolve_daemon_binary();
    let socket = default_socket_string();

    if cfg!(debug_assertions) {
        unload_launch_agent();
        if let Some(ref path) = binary {
            spawn_detached_daemon(path, &socket)?;
            if wait_for_daemon(&socket, 8) {
                return Ok(());
            }
        }
        return Err("Could not restart the Chronicle daemon.".into());
    }

    if let Some(ref path) = binary {
        if needs_plist_install(path) {
            let _ = run_daemon_install(path);
        }
    }

    if kickstart_launch_agent() && wait_for_daemon(&socket, 8) {
        return Ok(());
    }

    if let Some(path) = binary {
        spawn_detached_daemon(&path, &socket)?;
        if wait_for_daemon(&socket, 8) {
            return Ok(());
        }
    }

    Err("Could not restart the Chronicle daemon.".into())
}

fn launch_agent_plist_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join("Library/LaunchAgents/com.chronicle.daemon.plist"))
}

fn launch_agent_service_id() -> Option<String> {
    let uid = Command::new("id").arg("-u").output().ok()?;
    let uid = String::from_utf8_lossy(&uid.stdout).trim().to_string();
    Some(format!("gui/{uid}/com.chronicle.daemon"))
}

fn launch_agent_domain() -> Option<String> {
    let uid = Command::new("id").arg("-u").output().ok()?;
    let uid = String::from_utf8_lossy(&uid.stdout).trim().to_string();
    Some(format!("gui/{uid}"))
}

fn is_launch_agent_loaded() -> bool {
    let Some(service) = launch_agent_service_id() else {
        return false;
    };
    Command::new("launchctl")
        .args(["print", &service])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn unload_launch_agent() {
    let Some(service) = launch_agent_service_id() else {
        return;
    };
    let _ = Command::new("launchctl")
        .args(["bootout", &service])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn kickstart_launch_agent() -> bool {
    let Some(service) = launch_agent_service_id() else {
        return false;
    };
    Command::new("launchctl")
        .args(["kickstart", "-k", &service])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn plist_program_path(plist_path: &Path) -> Option<PathBuf> {
    let content = std::fs::read_to_string(plist_path).ok()?;
    let mut in_args = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "<key>ProgramArguments</key>" {
            in_args = true;
            continue;
        }
        if in_args && trimmed.starts_with("<string>") && trimmed.ends_with("</string>") {
            let inner = trimmed
                .trim_start_matches("<string>")
                .trim_end_matches("</string>");
            return Some(PathBuf::from(inner));
        }
        if in_args && trimmed == "</array>" {
            break;
        }
    }
    None
}

fn needs_plist_install(binary: &Path) -> bool {
    let Some(plist) = launch_agent_plist_path() else {
        return true;
    };
    if !plist.is_file() {
        return true;
    }
    let expected = binary.canonicalize().unwrap_or_else(|_| binary.to_path_buf());
    match plist_program_path(&plist) {
        Some(program) => {
            let current = program.canonicalize().unwrap_or(program);
            current != expected
        }
        None => true,
    }
}

fn run_daemon_install(binary: &Path) -> Result<(), String> {
    let install = Command::new(binary)
        .arg("install")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    match install {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Err(format!("daemon install failed: {stderr}"))
        }
        Err(e) => Err(format!("could not run chronicle-daemon install: {e}")),
    }
}

fn activate_launch_agent() -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    {
        return Err("launchctl only on macOS".into());
    }

    #[cfg(target_os = "macos")]
    {
        let plist = launch_agent_plist_path().ok_or_else(|| "no home directory".to_string())?;
        if !plist.is_file() {
            return Err("LaunchAgent plist missing".into());
        }

        let domain = launch_agent_domain().ok_or_else(|| "could not resolve uid".to_string())?;
        let plist_str = plist.to_string_lossy().to_string();

        unload_launch_agent();

        let bootstrap = Command::new("launchctl")
            .args(["bootstrap", &domain, &plist_str])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| e.to_string())?;
        if !bootstrap.success() {
            return Err("launchctl bootstrap failed".into());
        }

        if kickstart_launch_agent() {
            Ok(())
        } else {
            Err("launchctl kickstart failed".into())
        }
    }
}

fn spawn_detached_daemon(binary: &Path, socket: &str) -> Result<(), String> {
    let store = chronicle_config::default_store_path();
    Command::new(binary)
        .args([
            "start",
            "--socket",
            socket,
            "--store",
            &store.to_string_lossy(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn daemon: {e}"))?;
    Ok(())
}

fn wait_for_daemon(socket: &str, attempts: u32) -> bool {
    for _ in 0..attempts {
        if daemon_reachable(socket) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}
