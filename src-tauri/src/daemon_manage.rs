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

    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/release/chronicle-daemon");
    if dev.is_file() {
        return Some(dev);
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

    let install = Command::new(&binary).arg("install").output();
    match install {
        Ok(out) if out.status.success() => {}
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(format!("daemon install failed: {stderr}"));
        }
        Err(e) => return Err(format!("could not run chronicle-daemon install: {e}")),
    }

    if activate_launch_agent().is_err() {
        // launchd blocked (MDM, etc.) — run detached in user space instead.
        spawn_detached_daemon(&binary, &socket)?;
        if !wait_for_daemon(&socket, 12) {
            return Err(
                "Chronicle could not start the background service. Check Settings → Restart daemon."
                    .into(),
            );
        }
        return Ok(());
    }

    if !wait_for_daemon(&socket, 8) {
        spawn_detached_daemon(&binary, &socket)?;
        if !wait_for_daemon(&socket, 8) {
            return Err("Daemon installed but not responding. Try Settings → Restart daemon.".into());
        }
    }

    Ok(())
}

pub fn restart_daemon() -> Result<(), String> {
    let binary = resolve_daemon_binary();
    if let Some(ref path) = binary {
        let _ = Command::new(path).arg("install").status();
    }

    #[cfg(target_os = "macos")]
    {
        let uid = Command::new("id")
            .arg("-u")
            .output()
            .map_err(|e| e.to_string())?;
        let uid = String::from_utf8_lossy(&uid.stdout).trim().to_string();
        let label = format!("gui/{uid}/com.chronicle.daemon");
        let status = Command::new("launchctl")
            .args(["kickstart", "-k", &label])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() && wait_for_daemon(&default_socket_string(), 8) {
            return Ok(());
        }
    }

    if let Some(path) = binary {
        spawn_detached_daemon(&path, &default_socket_string())?;
        if wait_for_daemon(&default_socket_string(), 8) {
            return Ok(());
        }
    }

    Err("Could not restart the Chronicle daemon.".into())
}

fn activate_launch_agent() -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    {
        return Err("launchctl only on macOS".into());
    }

    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir().ok_or_else(|| "no home directory".to_string())?;
        let plist = home.join("Library/LaunchAgents/com.chronicle.daemon.plist");
        if !plist.is_file() {
            return Err("LaunchAgent plist missing".into());
        }

        let uid = Command::new("id")
            .arg("-u")
            .output()
            .map_err(|e| e.to_string())?;
        let uid = String::from_utf8_lossy(&uid.stdout).trim().to_string();
        let domain = format!("gui/{uid}");
        let plist_str = plist.to_string_lossy().to_string();

        let _ = Command::new("launchctl")
            .args(["bootout", &domain, &plist_str])
            .status();

        let bootstrap = Command::new("launchctl")
            .args(["bootstrap", &domain, &plist_str])
            .status()
            .map_err(|e| e.to_string())?;
        if !bootstrap.success() {
            return Err("launchctl bootstrap failed".into());
        }

        let kick = Command::new("launchctl")
            .args(["kickstart", "-k", &format!("{domain}/com.chronicle.daemon")])
            .status()
            .map_err(|e| e.to_string())?;
        if kick.success() {
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
