use chronicle_core::{CanonicalEvent, EventCategory};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, warn};

const POLL_INTERVAL: Duration = Duration::from_secs(2);

pub struct WindowFocusCollector;

impl WindowFocusCollector {
    pub async fn run(self, tx: mpsc::Sender<CanonicalEvent>) {
        debug!("window_focus collector started (polling every 2s)");
        let mut last_app: Option<String> = None;

        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            match get_frontmost_app().await {
                Ok(info) => {
                    let app_key = format!("{}|{}", info.name, info.bundle_id);
                    if last_app.as_ref() != Some(&app_key) {
                        let mut event = CanonicalEvent::new(
                            "chronicle-daemon",
                            EventCategory::Os,
                            "process.focus",
                        )
                        .with_project(&info.project_name());

                        let meta = event.metadata.as_object_mut().unwrap();
                        meta.insert("app_name".into(), info.name.clone().into());
                        meta.insert("bundle_id".into(), info.bundle_id.clone().into());
                        if let Some(title) = info.window_title {
                            meta.insert("window_title".into(), title.into());
                        }

                        if tx.send(event).await.is_err() {
                            warn!("window_focus: receiver dropped");
                            return;
                        }
                        last_app = Some(app_key);
                    }
                }
                Err(e) => debug!("window_focus: {e}"),
            }
        }
    }
}

struct AppInfo {
    name: String,
    bundle_id: String,
    window_title: Option<String>,
}

impl AppInfo {
    fn project_name(&self) -> String {
        self.name
            .split('.')
            .next()
            .unwrap_or(&self.name)
            .to_lowercase()
    }
}

async fn get_frontmost_app() -> Result<AppInfo, String> {
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            r#"tell application "System Events"
                set frontApp to first application process whose frontmost is true
                set appName to name of frontApp
                set bundleId to bundle identifier of frontApp
                return appName & "|||" & bundleId
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
    let bundle_id = parts[1].trim().to_string();
    let window_title = get_window_title().await;

    Ok(AppInfo {
        name,
        bundle_id,
        window_title,
    })
}

async fn get_window_title() -> Option<String> {
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            r#"tell application "System Events"
                set frontApp to first application process whose frontmost is true
                set appName to name of frontApp
            end tell
            tell application appName
                if (count of windows) > 0 then
                    return name of front window
                else
                    return ""
                end if
            end tell"#,
        ])
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
