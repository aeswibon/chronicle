use chronicle_core::CanonicalEvent;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::capture_status::CaptureStatus;
use crate::focus_emit::FocusEmitter;

use super::macos_focus::{self, CapturePermissions, FocusSample};

const RESTART_DELAY: Duration = Duration::from_secs(2);

pub struct WindowFocusCollector {
    capture_status: Arc<CaptureStatus>,
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
impl WindowFocusCollector {
    pub fn new(capture_status: Arc<CaptureStatus>) -> Self {
        Self { capture_status }
    }

    pub async fn run(
        self,
        tx: mpsc::Sender<CanonicalEvent>,
        focus: Arc<crate::focus_context::FocusContext>,
    ) {
        info!("window_focus collector started (NSWorkspace + AX/CGWindow monitor)");
        let mut emitter = FocusEmitter::default();
        loop {
            match self.run_monitor_session(&tx, &focus, &mut emitter).await {
                Ok(()) => warn!("focus monitor exited, restarting in {:?}", RESTART_DELAY),
                Err(e) => debug!("focus monitor session ended: {e}"),
            }
            self.capture_status.set_monitor_running(false);
            emitter.reset();
            tokio::time::sleep(RESTART_DELAY).await;
            if let Ok(Some(sample)) = tokio::task::spawn_blocking(macos_focus::query_snapshot).await
            {
                handle_sample(&tx, &focus, &mut emitter, &sample).await;
            }
        }
    }

    async fn run_monitor_session(
        &self,
        tx: &mpsc::Sender<CanonicalEvent>,
        focus: &Arc<crate::focus_context::FocusContext>,
        emitter: &mut FocusEmitter,
    ) -> Result<(), String> {
        let mut child = macos_focus::spawn_monitor()
            .await
            .ok_or("focus monitor helper unavailable")?;
        self.capture_status.set_monitor_running(true);
        let stdout = child.stdout.take().ok_or("focus monitor missing stdout")?;
        let mut reader = BufReader::new(stdout).lines();

        loop {
            tokio::select! {
                line = reader.next_line() => {
                    let line = line.map_err(|e| format!("read monitor stdout: {e}"))?;
                    let Some(line) = line else { break; };
                    let Some(sample) = macos_focus::parse_sample_line(line.as_bytes()) else { continue; };
                    let perms = CapturePermissions {
                        accessibility_trusted: sample.accessibility_trusted,
                        screen_capture_granted: sample.screen_capture_granted,
                        can_read_window_titles: sample.accessibility_trusted || sample.screen_capture_granted,
                    };
                    self.capture_status.update_from_sample(
                        &sample.name,
                        &sample.title_source,
                        &perms,
                        true,
                    );
                    handle_sample(tx, focus, emitter, &sample).await;
                }
                status = child.wait() => {
                    let _ = status.map_err(|e| format!("wait monitor: {e}"))?;
                    break;
                }
            }
        }
        Ok(())
    }
}

async fn handle_sample(
    tx: &mpsc::Sender<CanonicalEvent>,
    focus: &Arc<crate::focus_context::FocusContext>,
    emitter: &mut FocusEmitter,
    sample: &FocusSample,
) {
    emitter.update_focus_context(
        focus,
        &sample.name,
        &sample.bundle_id,
        sample.pid,
        sample.window_title.clone(),
    );
    let Some(event) = emitter.sample_to_event(
        &sample.name,
        &sample.bundle_id,
        sample.pid,
        sample.window_title.clone(),
        &sample.title_source,
        sample.timestamp_ms,
        None,
    ) else {
        return;
    };
    let _ = tx.send(event).await;
}
