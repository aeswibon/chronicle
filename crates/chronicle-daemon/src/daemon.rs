use crate::collectors;
use chronicle_core::CanonicalEvent;
use chronicle_ipc::{DaemonRequest, DaemonResponse, Server};
use chronicle_store::Store;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, mpsc};
use tracing::{error, info, warn};

pub struct Daemon {
    socket_path: String,
    store_path: String,
    watch_dirs: Vec<String>,
}

impl Daemon {
    pub fn new(socket_path: String, store_path: String, watch_dirs: Vec<String>) -> Self {
        Self {
            socket_path,
            store_path,
            watch_dirs,
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let expanded_store = shellexpand::tilde(&self.store_path).to_string();
        let store_path = Path::new(&expanded_store);

        if let Some(parent) = store_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let store = Store::open(store_path).map_err(|e| anyhow::anyhow!("store: {e}"))?;
        let store = Arc::new(Mutex::new(store));

        let server = Server::bind(&self.socket_path)
            .map_err(|e| anyhow::anyhow!("bind: {e}"))?;
        let started_at = Instant::now();
        let event_counter = Arc::new(AtomicU64::new(0));

        let (event_tx, mut event_rx) = mpsc::channel::<CanonicalEvent>(1024);

        let store_persist = store.clone();
        let counter = event_counter.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                if let Err(e) = store_persist.lock().await.insert_event(&event) {
                    error!("persist event: {e}");
                }
                counter.fetch_add(1, Ordering::Relaxed);
            }
        });

        let watch_dirs: Vec<_> = self
            .watch_dirs
            .iter()
            .map(|d| {
                let expanded = shellexpand::tilde(d).to_string();
                std::path::PathBuf::from(expanded)
            })
            .collect();

        let collectors: Vec<collectors::Collector> = vec![
            collectors::Collector::WindowFocus(
                collectors::window_focus::WindowFocusCollector,
            ),
            collectors::Collector::Filesystem(
                collectors::filesystem::FilesystemCollector::new(
                    if watch_dirs.is_empty() {
                        None
                    } else {
                        Some(watch_dirs.clone())
                    },
                ),
            ),
            collectors::Collector::Shell(collectors::shell::ShellHookCollector),
            collectors::Collector::Git(collectors::git::GitCollector::new(watch_dirs)),
        ];

        for collector in collectors {
            let tx = event_tx.clone();
            tokio::spawn(async move {
                collector.run(tx).await;
            });
        }

        info!(
            "Chronicle daemon ready on {} (store: {})",
            self.socket_path, expanded_store
        );

        loop {
            match server.accept().await {
                Ok(mut conn) => {
                    let store = store.clone();
                    let counter = event_counter.clone();
                    let started = started_at;
                    tokio::spawn(async move {
                        match conn.read_request().await {
                            Ok(req) => {
                                let store = store.lock().await;
                                let resp = match req {
                                    DaemonRequest::GetStatus => {
                                        let count = store.count_events().unwrap_or(0);
                                        DaemonResponse::Status {
                                            uptime_secs: started.elapsed().as_secs(),
                                            events_count: count,
                                            version: env!("CARGO_PKG_VERSION").into(),
                                        }
                                    }
                                    DaemonRequest::GetTimeline { since, until, limit } => {
                                        match store.query_events(since, until, limit) {
                                            Ok(events) => {
                                                let spans = events
                                                    .into_iter()
                                                    .map(|e| {
                                                        chronicle_core::Span::new(
                                                            chronicle_core::SpanType::Idle,
                                                            e.project.clone(),
                                                        )
                                                    })
                                                    .collect();
                                                DaemonResponse::Timeline { spans }
                                            }
                                            Err(e) => DaemonResponse::Error {
                                                code: 500,
                                                message: format!("query failed: {e}"),
                                            },
                                        }
                                    }
                                    DaemonRequest::EmitEvent { event } => {
                                        let event_id = event.id.to_string();
                                        if let Err(e) = store.insert_event(&event) {
                                            DaemonResponse::Error {
                                                code: 500,
                                                message: format!("insert failed: {e}"),
                                            }
                                        } else {
                                            counter.fetch_add(1, Ordering::Relaxed);
                                            DaemonResponse::Ack { event_id }
                                        }
                                    }
                                    _ => DaemonResponse::Error {
                                        code: 400,
                                        message: "unimplemented".into(),
                                    },
                                };
                                drop(store);
                                if let Err(e) = conn.send_response(resp).await {
                                    warn!("send response: {e}");
                                }
                            }
                            Err(e) => {
                                warn!("read request: {e}");
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("accept: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    }
}
