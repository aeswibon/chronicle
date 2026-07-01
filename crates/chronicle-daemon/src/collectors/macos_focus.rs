//! macOS front-app capture via compiled AppKit helper (NSWorkspace + AX/CGWindow).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tracing::{debug, warn};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FocusSample {
    pub event: String,
    pub name: String,
    pub bundle_id: String,
    pub pid: i32,
    pub window_title: Option<String>,
    pub title_source: String,
    pub timestamp_ms: i64,
    pub accessibility_trusted: bool,
    pub screen_capture_granted: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CapturePermissions {
    pub accessibility_trusted: bool,
    pub screen_capture_granted: bool,
    pub can_read_window_titles: bool,
}

fn built_helper_path() -> Option<PathBuf> {
    option_env!("CHRONICLE_FOCUS_MONITOR_HELPER")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
}

/// Resolve helper: env override → sibling of daemon binary → compile-time OUT_DIR.
pub fn helper_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CHRONICLE_FOCUS_MONITOR") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let sibling = parent.join("chronicle-focus-monitor");
            if sibling.is_file() {
                return Some(sibling);
            }
        }
    }
    built_helper_path()
}

/// Run helper in the interactive user session so Accessibility TCC applies (LaunchAgents lack AX).
#[cfg(target_os = "macos")]
fn run_helper(helper: &Path, args: &[&str]) -> std::io::Result<Output> {
    let uid = unsafe { libc::getuid() };
    let mut cmd = Command::new("launchctl");
    cmd.arg("asuser").arg(uid.to_string()).arg(helper);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.output()
}

#[cfg(not(target_os = "macos"))]
fn run_helper(helper: &Path, args: &[&str]) -> std::io::Result<Output> {
    let mut cmd = Command::new(helper);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.output()
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
fn spawn_helper(helper: &Path, args: &[&str]) -> std::io::Result<std::process::Child> {
    let uid = unsafe { libc::getuid() };
    let mut cmd = Command::new("launchctl");
    cmd.arg("asuser").arg(uid.to_string()).arg(helper);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
fn spawn_helper(helper: &Path, args: &[&str]) -> std::io::Result<std::process::Child> {
    let mut cmd = Command::new(helper);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
}

/// Copy the build-time helper next to the running daemon binary (release install).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn install_helper_beside_daemon() -> std::io::Result<PathBuf> {
    let src = built_helper_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "focus monitor helper not built",
        )
    })?;
    let exe = std::env::current_exe()?;
    let dest = exe
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "daemon parent dir"))?
        .join("chronicle-focus-monitor");
    std::fs::copy(&src, &dest)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms)?;
    }
    Ok(dest)
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn query_permissions() -> Option<CapturePermissions> {
    let helper = helper_path()?;
    let output = run_helper(&helper, &["permissions"]).ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

pub fn query_snapshot() -> Option<FocusSample> {
    let helper = helper_path()?;
    let output = run_helper(&helper, &["snapshot"]).ok()?;
    if !output.status.success() {
        return None;
    }
    parse_sample_line(&output.stdout)
}

pub async fn spawn_monitor() -> Option<tokio::process::Child> {
    let helper = helper_path()?;
    if !helper.is_file() {
        warn!("focus monitor helper missing: {}", helper.display());
        return None;
    }
    let helper_str = helper.to_string_lossy().into_owned();
    #[cfg(target_os = "macos")]
    {
        let uid = unsafe { libc::getuid() };
        let script = format!("exec launchctl asuser {uid} '{helper_str}' monitor");
        tokio::process::Command::new("/bin/sh")
            .args(["-c", &script])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .inspect_err(|e| warn!("failed to spawn focus monitor: {e}"))
            .ok()
    }
    #[cfg(not(target_os = "macos"))]
    {
        tokio::process::Command::new(&helper_str)
            .arg("monitor")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .inspect_err(|e| warn!("failed to spawn focus monitor: {e}"))
            .ok()
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn request_accessibility_prompt() -> bool {
    let Some(helper) = helper_path() else {
        return false;
    };
    run_helper(&helper, &["request-accessibility"])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn parse_sample_line(bytes: &[u8]) -> Option<FocusSample> {
    let line = std::str::from_utf8(bytes).ok()?.trim();
    if line.is_empty() {
        return None;
    }
    match serde_json::from_str::<FocusSample>(line) {
        Ok(sample) => Some(sample),
        Err(e) => {
            debug!("focus monitor JSON parse error: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sample_json() {
        let json = br#"{"event":"activation","name":"Cursor","bundle_id":"com.todesktop","pid":1,"window_title":"chronicle","title_source":"accessibility","timestamp_ms":1000,"accessibility_trusted":true,"screen_capture_granted":false}"#;
        let s = parse_sample_line(json).unwrap();
        assert_eq!(s.name, "Cursor");
        assert_eq!(s.title_source, "accessibility");
    }
}
