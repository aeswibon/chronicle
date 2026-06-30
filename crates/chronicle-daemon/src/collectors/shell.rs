use crate::project;
use chronicle_core::{CanonicalEvent, EventCategory};
use serde::Deserialize;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

const SHELL_HOOK_PORT: u16 = 9712;
const MAX_DATAGRAM_SIZE: usize = 4096;

pub struct ShellHookCollector;

#[derive(Debug, Deserialize)]
struct ShellCommand {
    cmd: String,
    exit_code: i32,
    dur: u64,
    cwd: String,
}

impl ShellHookCollector {
    pub async fn run(self, tx: mpsc::Sender<CanonicalEvent>) {
        let bind_addr = format!("127.0.0.1:{SHELL_HOOK_PORT}");
        let socket = match UdpSocket::bind(&bind_addr).await {
            Ok(s) => {
                info!("shell hook receiver listening on {bind_addr}");
                s
            }
            Err(e) => {
                error!("failed to bind UDP {bind_addr}: {e}");
                return;
            }
        };

        let mut buf = vec![0u8; MAX_DATAGRAM_SIZE];

        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    let data = &buf[..len];
                    match serde_json::from_slice::<ShellCommand>(data) {
                        Ok(cmd) => {
                            debug!(
                                "shell hook from {addr}: {} (dur={}, exit={})",
                                cmd.cmd, cmd.dur, cmd.exit_code
                            );

                            let event_type = if cmd.exit_code == 0 {
                                "command.completed"
                            } else {
                                "command.failed"
                            };

                            let source = cmd
                                .cmd
                                .split_whitespace()
                                .next()
                                .unwrap_or("shell")
                                .to_string();

                            let project_name = project::project_name_from_cwd(&cmd.cwd)
                                .unwrap_or_else(|| {
                                    cmd.cwd.rsplit('/').next().unwrap_or("unknown").to_string()
                                });

                            let mut event =
                                CanonicalEvent::new(&source, EventCategory::Shell, event_type)
                                    .with_project(&project_name)
                                    .with_duration(cmd.dur);

                            let meta = event.metadata.as_object_mut().unwrap();
                            meta.insert("command".into(), cmd.cmd.into());
                            meta.insert("exit_code".into(), cmd.exit_code.to_string().into());
                            meta.insert("cwd".into(), cmd.cwd.clone().into());
                            if let Some((_, root)) =
                                project::detect_project(std::path::Path::new(&cmd.cwd))
                            {
                                meta.insert("project_path".into(), root.to_string_lossy().into());
                            }

                            if tx.send(event).await.is_err() {
                                warn!("shell: receiver dropped");
                                return;
                            }
                        }
                        Err(e) => {
                            debug!("invalid shell hook payload: {e}");
                        }
                    }
                }
                Err(e) => {
                    error!("UDP recv error: {e}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
}
