//! Relay focus events from the GUI-session monitor into the daemon (LaunchAgents lack AX).

use chronicle_core::tab_session::TabSessionTracker;
use chronicle_core::{CanonicalEvent, EventCategory};
use chronicle_ipc::{Client, DaemonRequest};
use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct FocusSample {
    name: String,
    bundle_id: String,
    pid: i32,
    window_title: Option<String>,
    title_source: String,
    timestamp_ms: i64,
}

pub fn spawn_focus_relay(socket_path: String) {
    std::thread::spawn(move || run_focus_relay(&socket_path));
}

fn run_focus_relay(socket_path: &str) {
    let Some(helper) = focus_monitor_path() else {
        eprintln!("chronicle focus relay: monitor helper not found");
        return;
    };
    let mut tracker = TabSessionTracker::default();
    loop {
        if let Ok(mut child) = Command::new(&helper)
            .arg("monitor")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                let rt = tokio::runtime::Runtime::new().expect("focus relay runtime");
                for line in reader.lines().flatten() {
                    let Ok(sample) = serde_json::from_str::<FocusSample>(&line) else {
                        continue;
                    };
                    let Some(change) =
                        tracker.observe(&sample.name, &sample.bundle_id, sample.window_title.as_deref())
                    else {
                        continue;
                    };
                    let mut event =
                        CanonicalEvent::new(&sample.name, EventCategory::Os, "process.focus");
                    event.timestamp = sample.timestamp_ms;
                    let meta = event.metadata.as_object_mut().unwrap();
                    meta.insert("app_name".into(), sample.name.clone().into());
                    meta.insert("bundle_id".into(), sample.bundle_id.clone().into());
                    meta.insert("pid".into(), sample.pid.into());
                    meta.insert("title_source".into(), sample.title_source.clone().into());
                    meta.insert("capture_backend".into(), "nsworkspace".into());
                    meta.insert("capture_relay".into(), "chronicle-ui".into());
                    meta.insert("tab_session_key".into(), change.tab_session_key.clone().into());
                    meta.insert("tab_title".into(), change.tab_title.clone().into());
                    meta.insert(
                        "focus_kind".into(),
                        if change.app_changed { "app" } else { "tab" }.into(),
                    );
                    if let Some(title) = sample.window_title.clone() {
                        meta.insert("window_title".into(), title.into());
                    }
                    let socket = socket_path.to_string();
                    rt.block_on(async move {
                        if let Ok(mut client) = Client::connect(&socket).await {
                            let _ = client.request(DaemonRequest::EmitEvent { event }).await;
                        }
                    });
                }
            }
            let _ = child.wait();
        }
        tracker.reset();
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn focus_monitor_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CHRONICLE_FOCUS_MONITOR") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    let daemon = crate::daemon_manage::resolve_daemon_binary()?;
    let sibling = daemon.parent()?.join("chronicle-focus-monitor");
    sibling.is_file().then_some(sibling)
}
