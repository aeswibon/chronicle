use chronicle_core::{CanonicalEvent, EventCategory};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::event_filter;
use crate::project;

const POLL_INTERVAL: Duration = Duration::from_secs(2);

pub struct WindowFocusCollector;

impl WindowFocusCollector {
    pub async fn run(self, tx: mpsc::Sender<CanonicalEvent>) {
        debug!("window_focus collector started (polling every 2s)");
        let mut last_app: Option<String> = None;

        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            let info = match tokio::task::spawn_blocking(get_frontmost_app_sync).await {
                Ok(Ok(info)) => info,
                Ok(Err(e)) => {
                    debug!("window_focus: {e}");
                    continue;
                }
                Err(e) => {
                    debug!("window_focus join: {e}");
                    continue;
                }
            };

            let app_key = info.name.to_lowercase();
            if last_app.as_ref() == Some(&app_key) {
                continue;
            }

            let mut event = CanonicalEvent::new(&info.name, EventCategory::Os, "process.focus");

            if let Some(title) = info.window_title.as_deref() {
                if let Some((project, root)) = project::detect_project_from_title(title) {
                    event = event.with_project(&project);
                    let meta = event.metadata.as_object_mut().unwrap();
                    meta.insert("project_path".into(), root.to_string_lossy().into());
                }
            }

            let meta = event.metadata.as_object_mut().unwrap();
            meta.insert("app_name".into(), info.name.clone().into());
            meta.insert("bundle_id".into(), info.bundle_id.clone().into());
            if let Some(title) = info.window_title {
                meta.insert("window_title".into(), title.into());
            }

            if !event_filter::should_record(&event) {
                continue;
            }

            if tx.send(event).await.is_err() {
                warn!("window_focus: receiver dropped");
                return;
            }
            last_app = Some(app_key);
        }
    }
}

struct AppInfo {
    name: String,
    bundle_id: String,
    window_title: Option<String>,
}

fn get_frontmost_app_sync() -> Result<AppInfo, String> {
    // System Events only — never `tell application appName` (triggers Choose Application).
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            r#"tell application "System Events"
                set frontApp to first application process whose frontmost is true
                set appName to name of frontApp
                set bundleId to bundle identifier of frontApp
                set winTitle to ""
                try
                    if (count of windows of frontApp) > 0 then
                        set winTitle to name of front window of frontApp
                    end if
                end try
                return appName & "|||" & bundleId & "|||" & winTitle
            end tell"#,
        ])
        .output()
        .map_err(|e| format!("osascript exec: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "osascript: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parts: Vec<&str> = stdout.split("|||").collect();
    if parts.len() < 2 {
        return Err("unexpected osascript output".into());
    }

    let name = parts[0].trim().to_string();
    let mut bundle_id = parts[1].trim().to_string();
    if bundle_id == "missing value" {
        bundle_id.clear();
    }
    let window_title = parts
        .get(2)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(AppInfo {
        name,
        bundle_id,
        window_title,
    })
}
