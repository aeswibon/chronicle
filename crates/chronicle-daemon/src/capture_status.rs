//! Latest macOS capture health for status IPC.

use crate::collectors::macos_focus::CapturePermissions;
use chronicle_ipc::MacosCaptureStatus;
use std::sync::Mutex;

#[derive(Debug, Clone, Default)]
pub struct CaptureStatusSnapshot {
    pub monitor_running: bool,
    pub frontmost_app: Option<String>,
    pub title_source: Option<String>,
    pub permissions: CapturePermissions,
}

impl CaptureStatusSnapshot {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn to_ipc(&self) -> MacosCaptureStatus {
        MacosCaptureStatus {
            monitor_running: self.monitor_running,
            frontmost_app: self.frontmost_app.clone(),
            title_source: self.title_source.clone(),
            accessibility_trusted: self.permissions.accessibility_trusted,
            screen_capture_granted: self.permissions.screen_capture_granted,
            can_read_window_titles: self.permissions.can_read_window_titles,
        }
    }
}

#[derive(Default)]
pub struct CaptureStatus {
    inner: Mutex<CaptureStatusSnapshot>,
}

impl CaptureStatus {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn seed_permissions(&self, permissions: CapturePermissions) {
        if let Ok(mut g) = self.inner.lock() {
            g.permissions = permissions;
        }
    }

    pub fn update_from_sample(
        &self,
        name: &str,
        title_source: &str,
        permissions: &CapturePermissions,
        monitor_running: bool,
    ) {
        if let Ok(mut g) = self.inner.lock() {
            g.monitor_running = monitor_running;
            g.frontmost_app = Some(name.to_string());
            g.title_source = Some(title_source.to_string());
            g.permissions = permissions.clone();
        }
    }

    pub fn set_monitor_running(&self, running: bool) {
        if let Ok(mut g) = self.inner.lock() {
            g.monitor_running = running;
        }
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn snapshot(&self) -> CaptureStatusSnapshot {
        self.inner.lock().map(|g| g.clone()).unwrap_or_default()
    }
}
