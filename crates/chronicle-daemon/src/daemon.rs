use crate::collectors;
use crate::span_processor::SpanProcessor;
use chronicle_core::CanonicalEvent;
use chronicle_ipc::{DaemonRequest, DaemonResponse, Server};
use chronicle_store::Store;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::broadcast;
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

        let (event_tx, event_rx) = mpsc::channel::<CanonicalEvent>(1024);
        let (broadcast_tx, _) = broadcast::channel::<CanonicalEvent>(256);

        let store_persist = store.clone();
        let counter = event_counter.clone();
        let broadcast_tx_clone = broadcast_tx.clone();
        tokio::spawn(async move {
            process_events(event_rx, store_persist, counter, broadcast_tx_clone).await;
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

        let mut sigint = signal(SignalKind::interrupt())?;
        let mut sigterm = signal(SignalKind::terminate())?;

        info!(
            "Chronicle daemon ready on {} (store: {})",
            self.socket_path, expanded_store
        );

        loop {
            tokio::select! {
                _ = sigint.recv() => {
                    info!("received SIGINT, shutting down");
                    break;
                }
                _ = sigterm.recv() => {
                    info!("received SIGTERM, shutting down");
                    break;
                }
                result = server.accept() => {
                    match result {
                        Ok(mut conn) => {
                            let store = store.clone();
                            let counter = event_counter.clone();
                            let started = started_at;
                            let broadcast_rx = broadcast_tx.subscribe();
                            tokio::spawn(async move {
                                handle_connection(
                                    &mut conn,
                                    &store,
                                    &counter,
                                    started,
                                    broadcast_rx,
                                )
                                .await;
                            });
                        }
                        Err(e) => {
                            error!("accept error: {e}");
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        }

        info!("Chronicle daemon stopped");
        Ok(())
    }
}

async fn process_events(
    mut rx: mpsc::Receiver<CanonicalEvent>,
    store: Arc<Mutex<Store>>,
    counter: Arc<AtomicU64>,
    broadcast_tx: broadcast::Sender<CanonicalEvent>,
) {
    let mut span_processor = SpanProcessor::new();

    while let Some(event) = rx.recv().await {
        if let Err(e) = store.lock().await.insert_event(&event) {
            error!("persist event: {e}");
        }

        if let Some(span) = span_processor.process(&event) {
            if let Err(e) = store.lock().await.insert_span(&span) {
                error!("persist span: {e}");
            }
            info!(
                "session closed: {:?} ({:.1}m, {} events)",
                span.span_type,
                span.duration_ms.unwrap_or(0) as f64 / 60000.0,
                span.event_count
            );
        }

        counter.fetch_add(1, Ordering::Relaxed);
        let _ = broadcast_tx.send(event);
    }
}

async fn handle_connection(
    conn: &mut chronicle_ipc::Connection,
    store: &Arc<Mutex<Store>>,
    _counter: &Arc<AtomicU64>,
    started: Instant,
    mut broadcast_rx: broadcast::Receiver<CanonicalEvent>,
) {
    match conn.read_request().await {
        Ok(req) => match req {
            DaemonRequest::Subscribe { .. } => {
                loop {
                    match broadcast_rx.recv().await {
                        Ok(event) => {
                            if conn
                                .send_response(DaemonResponse::Event { event })
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    }
                }
            }
            _ => {
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
        },
        Err(e) => {
            warn!("read request: {e}");
        }
    }
}
