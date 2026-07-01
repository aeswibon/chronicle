//! Front window title for the focused app (macOS).

#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub fn window_title_for_pid(pid: i32) -> Option<String> {
    if pid <= 0 {
        return None;
    }
    let helper = option_env!("CHRONICLE_FRONT_WINDOW_HELPER")?;
    let output = std::process::Command::new(helper)
        .arg(pid.to_string())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let title = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if title.is_empty() {
        None
    } else {
        Some(title)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn window_title_for_pid(_pid: i32) -> Option<String> {
    None
}
