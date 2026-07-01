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
    #[cfg(target_os = "macos")]
    {
        get_frontmost_app_macos()
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("window_focus collector is only supported on macOS".into())
    }
}

#[cfg(target_os = "macos")]
fn get_frontmost_app_macos() -> Result<AppInfo, String> {
    use lsappinfo_parse::{parse_front_asn, parse_lsappinfo_field};

    // lsappinfo uses LaunchServices — no Accessibility / Automation permission prompt.
    // (AppleScript + System Events triggers "control this computer" for the daemon binary.)
    let front = std::process::Command::new("/usr/bin/lsappinfo")
        .arg("front")
        .output()
        .map_err(|e| format!("lsappinfo front: {e}"))?;
    if !front.status.success() {
        return Err(format!(
            "lsappinfo front: {}",
            String::from_utf8_lossy(&front.stderr)
        ));
    }

    let asn = parse_front_asn(&String::from_utf8_lossy(&front.stdout))
        .ok_or_else(|| "lsappinfo front: could not parse ASN".to_string())?;

    let info = std::process::Command::new("/usr/bin/lsappinfo")
        .args(["info", "-only", "name,bundleID", &asn])
        .output()
        .map_err(|e| format!("lsappinfo info: {e}"))?;
    if !info.status.success() {
        return Err(format!(
            "lsappinfo info: {}",
            String::from_utf8_lossy(&info.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&info.stdout);
    let name = parse_lsappinfo_field(&stdout, "LSDisplayName")
        .ok_or_else(|| "lsappinfo: missing app name".to_string())?;
    let bundle_id = parse_lsappinfo_field(&stdout, "CFBundleIdentifier").unwrap_or_default();

    Ok(AppInfo {
        name,
        bundle_id,
        window_title: None,
    })
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod lsappinfo_parse {
    pub fn parse_front_asn(stdout: &str) -> Option<String> {
        for line in stdout.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("ASN:") {
                let asn = format!("ASN:{rest}");
                return Some(asn.trim_end_matches(':').to_string());
            }
        }
        None
    }

    pub fn parse_lsappinfo_field(stdout: &str, key: &str) -> Option<String> {
        let needle = format!("\"{key}\"=");
        for line in stdout.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix(&needle) {
                return parse_quoted_value(rest);
            }
        }
        None
    }

    fn parse_quoted_value(raw: &str) -> Option<String> {
        let raw = raw.trim();
        if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
            return Some(raw[1..raw.len() - 1].to_string());
        }
        if raw == "[ NULL ]" || raw.is_empty() {
            return None;
        }
        Some(raw.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::lsappinfo_parse::{parse_front_asn, parse_lsappinfo_field};

    #[test]
    fn parse_front_asn_from_lsappinfo_output() {
        let sample = "ASN:0x0-0x9c89c8:\n\"Safari\" ASN:0x0-0x9c89c8: (in front)\n";
        assert_eq!(
            parse_front_asn(sample),
            Some("ASN:0x0-0x9c89c8".to_string())
        );
    }

    #[test]
    fn parse_lsappinfo_name_and_bundle() {
        let sample = "\"LSDisplayName\"=\"Safari\"\n\"CFBundleIdentifier\"=\"com.apple.Safari\"\n";
        assert_eq!(
            parse_lsappinfo_field(sample, "LSDisplayName"),
            Some("Safari".to_string())
        );
        assert_eq!(
            parse_lsappinfo_field(sample, "CFBundleIdentifier"),
            Some("com.apple.Safari".to_string())
        );
    }
}
