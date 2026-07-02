use crate::capture_status::CaptureStatus;
use crate::collectors;
use crate::event_filter;
use crate::focus_context::{self, FocusContext};
use crate::maintenance;
use crate::project;
use crate::span_processor::SpanProcessor;
use chronicle_core::{CanonicalEvent, Span, SpanType};
use chronicle_ipc::{DaemonRequest, DaemonResponse, Server};
use chronicle_store::Store;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::broadcast;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

struct PipelineState {
    span_processor: SpanProcessor,
    rule_engine: crate::rule_engine::RuleEngine,
}

impl PipelineState {
    fn new() -> Self {
        Self {
            span_processor: SpanProcessor::new(),
            rule_engine: crate::rule_engine::RuleEngine::new(),
        }
    }

    fn annotate_active_spans(&self, spans: &mut [Span]) {
        for span in spans.iter_mut() {
            self.rule_engine.annotate_span(span);
        }
    }
}

fn supplement_active_spans(active: &mut Vec<Span>, focus_ctx: &FocusContext) {
    if !active.is_empty() {
        return;
    }
    if let Some(snap) = focus_ctx.snapshot() {
        if let Some(span) = focus_context::open_span_from_focus(&snap) {
            active.push(span);
        }
    }
}

async fn active_spans_for_timeline_async(
    pipeline: &Arc<Mutex<PipelineState>>,
    focus_ctx: &FocusContext,
) -> Vec<Span> {
    let mut active = {
        let p = pipeline.lock().await;
        p.span_processor.active_spans()
    };
    {
        let p = pipeline.lock().await;
        p.annotate_active_spans(&mut active);
    }
    supplement_active_spans(&mut active, focus_ctx);
    active
}

fn find_span_by_id(active: &[Span], id: &str) -> Option<Span> {
    if let Ok(uuid) = uuid::Uuid::parse_str(id) {
        if let Some(span) = active.iter().find(|s| s.id == uuid) {
            return Some(span.clone());
        }
    }
    active.iter().find(|s| s.id.to_string() == id).cloned()
}

fn merge_timeline_spans(
    mut stored: Vec<Span>,
    active: Vec<Span>,
    since: i64,
    limit: u32,
) -> Vec<Span> {
    let active_ids: HashSet<_> = active.iter().map(|s| s.id).collect();
    stored.retain(|s| !active_ids.contains(&s.id));
    let mut all: Vec<Span> = active
        .into_iter()
        .chain(stored)
        .filter(|s| {
            s.started_at >= since && (s.ended_at.is_none() || s.span_type != SpanType::Idle)
        })
        .collect();
    all.sort_by_key(|b| std::cmp::Reverse(b.started_at));
    all.truncate(limit as usize);
    all
}

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

        let lock_path = store_path
            .parent()
            .unwrap_or_else(|| Path::new("/tmp"))
            .join("daemon.lock");
        let _lock = crate::singleton::DaemonLock::acquire(&lock_path)?;

        let store = Store::open(store_path).map_err(|e| anyhow::anyhow!("store: {e}"))?;
        if let Some(days) = chronicle_config::load().privacy.retention_days {
            let cutoff = chrono::Utc::now().timestamp_millis() - i64::from(days) * 86_400_000;
            match store.prune_before(cutoff) {
                Ok((events, spans)) if events + spans > 0 => {
                    info!(
                        "retention: pruned {events} events and {spans} spans older than {days} days"
                    );
                }
                Err(e) => warn!("retention prune failed: {e}"),
                _ => {}
            }
        }
        let store = Arc::new(Mutex::new(store));

        maintenance::spawn_maintenance_tasks(store.clone());

        let watch_dirs: Vec<_> = self
            .watch_dirs
            .iter()
            .map(|d| crate::watch_dirs::expand_path(d))
            .collect();
        let watch_dirs = crate::watch_dirs::resolve_watch_dirs(&watch_dirs);

        let server = Server::bind(&self.socket_path).map_err(|e| anyhow::anyhow!("bind: {e}"))?;
        let started_at = Instant::now();
        let event_counter = Arc::new(AtomicU64::new(0));

        let focus_ctx = Arc::new(FocusContext::default());
        let capture_status = Arc::new(CaptureStatus::default());
        #[cfg(target_os = "macos")]
        {
            if let Some(perms) = collectors::macos_focus::query_permissions() {
                capture_status.seed_permissions(perms);
            }
        }
        let pipeline = Arc::new(Mutex::new(PipelineState::new()));
        let capture_status_ipc = capture_status.clone();
        let (event_tx, event_rx) = mpsc::channel::<CanonicalEvent>(1024);
        let (broadcast_tx, _) = broadcast::channel::<CanonicalEvent>(256);

        let http_tx = event_tx.clone();
        tokio::spawn(async move {
            crate::http_ingress::run(9713, http_tx).await;
        });

        let store_persist = store.clone();
        let counter = event_counter.clone();
        let broadcast_tx_clone = broadcast_tx.clone();
        let pipeline_events = pipeline.clone();
        let focus_ctx_events = focus_ctx.clone();
        tokio::spawn(async move {
            process_events(
                event_rx,
                store_persist,
                counter,
                broadcast_tx_clone,
                pipeline_events,
                focus_ctx_events,
            )
            .await;
        });

        #[cfg(target_os = "macos")]
        {
            if let Err(e) = collectors::macos_focus::install_helper_beside_daemon() {
                warn!("focus monitor helper install skipped: {e}");
            }
        }

        let collector_cfg = chronicle_config::load().collectors;

        let mut collectors: Vec<collectors::Collector> = Vec::new();
        if collector_cfg.window_focus {
            collectors.push(collectors::Collector::WindowFocus(
                collectors::window_focus::WindowFocusCollector::new(capture_status.clone()),
            ));
        }
        if collector_cfg.filesystem {
            collectors.push(collectors::Collector::Filesystem(
                collectors::filesystem::FilesystemCollector::new(Some(watch_dirs.clone())),
            ));
        }
        if collector_cfg.shell {
            collectors.push(collectors::Collector::Shell(
                collectors::shell::ShellHookCollector,
            ));
        }
        if collector_cfg.git {
            collectors.push(collectors::Collector::Git(
                collectors::git::GitCollector::new(watch_dirs.clone()),
            ));
        }

        if collectors.is_empty() {
            warn!(
                "all collectors disabled in config — only emit_event / IPC events will be recorded"
            );
        }

        for collector in collectors {
            let tx = event_tx.clone();
            let focus = focus_ctx.clone();
            tokio::spawn(async move {
                collector.run(tx, focus).await;
            });
        }

        let store_bootstrap = store.clone();
        let bootstrap_dirs = watch_dirs.clone();
        tokio::spawn(async move {
            let needs_scan = {
                let guard = store_bootstrap.lock().await;
                guard.count_projects().unwrap_or(0) == 0
            };

            if needs_scan {
                let repos = tokio::task::spawn_blocking({
                    let dirs = bootstrap_dirs.clone();
                    move || crate::project_bootstrap::discover_repos(&dirs)
                })
                .await
                .unwrap_or_default();

                let guard = store_bootstrap.lock().await;
                crate::project_bootstrap::apply_discovered_repos(&guard, &repos);
            }

            let guard = store_bootstrap.lock().await;
            crate::project_bootstrap::bootstrap_projects_light(&guard);
        });

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
                            let event_tx_conn = event_tx.clone();
                            let pipeline_conn = pipeline.clone();
                            let focus_conn = focus_ctx.clone();
                            let capture_conn = capture_status_ipc.clone();
                            tokio::spawn(async move {
                                handle_connection(
                                    &mut conn,
                                    &store,
                                    &counter,
                                    started,
                                    broadcast_rx,
                                    event_tx_conn,
                                    pipeline_conn,
                                    focus_conn,
                                    capture_conn,
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
    pipeline: Arc<Mutex<PipelineState>>,
    focus_ctx: Arc<FocusContext>,
) {
    while let Some(mut event) = rx.recv().await {
        event_filter::sanitize_event(&mut event);
        if !event_filter::should_record(&event) {
            continue;
        }

        if event.category == chronicle_core::EventCategory::Os && event.r#type == "process.focus" {
            crate::focus_emit::ensure_tab_session_meta(&mut event);
        }

        focus_ctx.update_from_event(&event);
        let focus = focus_ctx.snapshot();
        if !focus_context::applies_to_focus(&event, focus.as_ref()) {
            continue;
        }

        let mut closed_spans = Vec::new();
        if event.category == chronicle_core::EventCategory::Os
            && event.r#type == "process.focus"
            && event.metadata.get("focus_kind").and_then(|v| v.as_str()) == Some("app")
        {
            let mut guard = pipeline.lock().await;
            closed_spans = guard.span_processor.close_all(event.timestamp);
            drop(guard);
        }

        let closed_span = {
            let mut guard = pipeline.lock().await;
            guard.rule_engine.process(&mut event);
            chronicle_ai::enrich_event(&mut event);

            {
                let store_guard = store.lock().await;
                if let Err(e) = store_guard.insert_event(&event) {
                    error!("persist event: {e}");
                } else if let Some(ref name) = event.project {
                    if let Some(path) = project::project_path_from_event(name, &event.metadata) {
                        if let Err(e) =
                            store_guard.upsert_project(name, &path, None, event.timestamp)
                        {
                            error!("upsert project: {e}");
                        }
                    }
                }
            }

            guard.span_processor.process(&event)
        };

        if let Some(span) = closed_span {
            closed_spans.push(span);
        }

        for mut span in closed_spans {
            let guard = pipeline.lock().await;
            guard.rule_engine.annotate_span(&mut span);
            drop(guard);
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

#[allow(clippy::too_many_arguments)]
async fn handle_connection(
    conn: &mut chronicle_ipc::Connection,
    store: &Arc<Mutex<Store>>,
    _counter: &Arc<AtomicU64>,
    started: Instant,
    mut broadcast_rx: broadcast::Receiver<CanonicalEvent>,
    event_tx: mpsc::Sender<CanonicalEvent>,
    pipeline: Arc<Mutex<PipelineState>>,
    focus_ctx: Arc<FocusContext>,
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] capture_status: Arc<
        CaptureStatus,
    >,
) {
    match conn.read_request().await {
        Ok(req) => match req {
            DaemonRequest::Subscribe { .. } => loop {
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
            },
            _ => {
                let resp = match req {
                    DaemonRequest::SummarizeDay { since, until } => {
                        let until =
                            until.unwrap_or_else(|| chrono::Local::now().timestamp_millis());
                        match maintenance::summarize_and_persist_day(
                            Arc::clone(store),
                            since,
                            until,
                        )
                        .await
                        {
                            Ok((summary, source, ai_error, session)) => DaemonResponse::DailySummary {
                                summary,
                                session,
                                source: Some(source),
                                ai_error,
                            },
                            Err(message) => DaemonResponse::Error { code: 500, message },
                        }
                    }
                    DaemonRequest::PruneNoiseEvents => {
                        let guard = store.lock().await;
                        match guard.prune_noise_events() {
                            Ok(deleted) => DaemonResponse::MaintenanceResult {
                                events_deleted: deleted,
                                spans_deleted: 0,
                                sessions_deleted: 0,
                            },
                            Err(e) => DaemonResponse::Error {
                                code: 500,
                                message: format!("prune failed: {e}"),
                            },
                        }
                    }
                    DaemonRequest::PurgeCaptureTimeline => {
                        let guard = store.lock().await;
                        match guard.purge_capture_timeline() {
                            Ok((events, spans, sessions)) => DaemonResponse::MaintenanceResult {
                                events_deleted: events,
                                spans_deleted: spans,
                                sessions_deleted: sessions,
                            },
                            Err(e) => DaemonResponse::Error {
                                code: 500,
                                message: format!("purge failed: {e}"),
                            },
                        }
                    }
                    other => {
                        let guard = store.lock().await;
                        match other {
                            DaemonRequest::GetStatus => {
                                let count = guard.count_events().unwrap_or(0);
                                #[cfg(target_os = "macos")]
                                let macos_capture = Some(capture_status.snapshot().to_ipc());
                                #[cfg(not(target_os = "macos"))]
                                let macos_capture = None;
                                DaemonResponse::Status {
                                    uptime_secs: started.elapsed().as_secs(),
                                    events_count: count,
                                    version: env!("CARGO_PKG_VERSION").into(),
                                    macos_capture,
                                }
                            }
                            DaemonRequest::RequestMacosAccessibility => {
                                #[cfg(target_os = "macos")]
                                {
                                    let _ = collectors::macos_focus::request_accessibility_prompt();
                                    if let Some(perms) =
                                        collectors::macos_focus::query_permissions()
                                    {
                                        capture_status.seed_permissions(perms.clone());
                                    }
                                    DaemonResponse::MacosCapture {
                                        status: capture_status.snapshot().to_ipc(),
                                    }
                                }
                                #[cfg(not(target_os = "macos"))]
                                {
                                    DaemonResponse::Error {
                                        code: 400,
                                        message:
                                            "macOS capture permissions are only available on macOS"
                                                .into(),
                                    }
                                }
                            }
                            DaemonRequest::GetTimeline {
                                since,
                                until,
                                limit,
                            } => match guard.query_spans(since, until, limit) {
                                Ok(stored) => {
                                    drop(guard);
                                    let active =
                                        active_spans_for_timeline_async(&pipeline, &focus_ctx)
                                            .await;
                                    DaemonResponse::Timeline {
                                        spans: merge_timeline_spans(stored, active, since, limit),
                                    }
                                }
                                Err(e) => DaemonResponse::Error {
                                    code: 500,
                                    message: format!("query failed: {e}"),
                                },
                            },
                            DaemonRequest::GetEvents {
                                since,
                                until,
                                limit,
                            } => match guard.query_activity_events(since, until, limit) {
                                Ok(events) => DaemonResponse::TimelineEvents { events },
                                Err(e) => DaemonResponse::Error {
                                    code: 500,
                                    message: format!("query failed: {e}"),
                                },
                            },
                            DaemonRequest::Search { query, mode, limit } => {
                                let query = match mode {
                                    chronicle_ipc::SearchMode::Semantic => {
                                        chronicle_ai::expand_search_query(&query)
                                    }
                                    chronicle_ipc::SearchMode::Keyword => query,
                                };
                                match guard.search_events(&query, limit) {
                                    Ok(events) => DaemonResponse::TimelineEvents { events },
                                    Err(e) => DaemonResponse::Error {
                                        code: 500,
                                        message: format!("search failed: {e}"),
                                    },
                                }
                            }
                            DaemonRequest::ListProjects { limit } => {
                                match guard.query_projects(limit) {
                                    Ok(projects) => DaemonResponse::Projects { projects },
                                    Err(e) => DaemonResponse::Error {
                                        code: 500,
                                        message: format!("projects query failed: {e}"),
                                    },
                                }
                            }
                            DaemonRequest::GetProjectContext {
                                project,
                                since,
                                limit,
                            } => {
                                let project_record =
                                    guard.query_project_by_name(&project).ok().flatten();
                                let stored = guard
                                    .query_spans_for_project(&project, since, limit)
                                    .unwrap_or_default();
                                let events = guard
                                    .query_activity_events_for_project(&project, since, None, limit)
                                    .unwrap_or_default();
                                drop(guard);
                                let mut active = {
                                    let p = pipeline.lock().await;
                                    let mut spans = p.span_processor.active_spans();
                                    p.annotate_active_spans(&mut spans);
                                    spans
                                        .into_iter()
                                        .filter(|s| {
                                            s.project.as_deref() == Some(project.as_str())
                                        })
                                        .collect::<Vec<_>>()
                                };
                                supplement_active_spans(&mut active, &focus_ctx);
                                DaemonResponse::ProjectContext {
                                    project: project_record,
                                    spans: merge_timeline_spans(stored, active, since, limit),
                                    events,
                                }
                            }
                            DaemonRequest::GetSpan { id, event_limit } => {
                                match guard.query_span_by_id(&id) {
                                    Ok(Some(span)) => {
                                        let since = span.started_at;
                                        let until = span.ended_at.unwrap_or(i64::MAX);
                                        let events = if let Some(ref project) = span.project {
                                            guard
                                                .query_activity_events_for_project(
                                                    project,
                                                    since,
                                                    Some(until),
                                                    event_limit,
                                                )
                                                .unwrap_or_default()
                                        } else {
                                            guard
                                                .query_events(since, Some(until), event_limit)
                                                .unwrap_or_default()
                                        };
                                        DaemonResponse::SpanDetail { span, events }
                                    }
                                    Ok(None) => {
                                        drop(guard);
                                        let active =
                                            active_spans_for_timeline_async(&pipeline, &focus_ctx)
                                                .await;
                                        if let Some(span) = find_span_by_id(&active, &id) {
                                            let guard = store.lock().await;
                                            let since = span.started_at;
                                            let until = span.ended_at.unwrap_or(i64::MAX);
                                            let events = if let Some(ref project) = span.project {
                                                guard
                                                    .query_activity_events_for_project(
                                                        project,
                                                        since,
                                                        Some(until),
                                                        event_limit,
                                                    )
                                                    .unwrap_or_default()
                                            } else {
                                                guard
                                                    .query_events(since, Some(until), event_limit)
                                                    .unwrap_or_default()
                                            };
                                            DaemonResponse::SpanDetail { span, events }
                                        } else {
                                            DaemonResponse::Error {
                                                code: 404,
                                                message: format!("span not found: {id}"),
                                            }
                                        }
                                    }
                                    Err(e) => DaemonResponse::Error {
                                        code: 500,
                                        message: format!("span query failed: {e}"),
                                    },
                                }
                            }
                            DaemonRequest::GetErrors { since, limit } => {
                                match guard.query_errors(since, limit) {
                                    Ok(events) => DaemonResponse::TimelineEvents { events },
                                    Err(e) => DaemonResponse::Error {
                                        code: 500,
                                        message: format!("errors query failed: {e}"),
                                    },
                                }
                            }
                            DaemonRequest::GetSessions { since, until } => {
                                match guard.query_sessions(since, until) {
                                    Ok(sessions) => DaemonResponse::Sessions { sessions },
                                    Err(e) => DaemonResponse::Error {
                                        code: 500,
                                        message: format!("sessions query failed: {e}"),
                                    },
                                }
                            }
                            DaemonRequest::DeleteSession { id } => {
                                let parsed = uuid::Uuid::parse_str(&id);
                                match parsed {
                                    Ok(session_id) => match guard.delete_session(&session_id) {
                                        Ok(0) => DaemonResponse::Error {
                                            code: 404,
                                            message: format!("session not found: {id}"),
                                        },
                                        Ok(_) => DaemonResponse::Ack { event_id: id },
                                        Err(e) => DaemonResponse::Error {
                                            code: 500,
                                            message: format!("delete session failed: {e}"),
                                        },
                                    },
                                    Err(e) => DaemonResponse::Error {
                                        code: 400,
                                        message: format!("invalid session id: {e}"),
                                    },
                                }
                            }
                            DaemonRequest::ListPlugins => DaemonResponse::Plugins {
                                plugins: chronicle_plugin::discover_plugins(),
                            },
                            DaemonRequest::GetConfig => {
                                let cfg = chronicle_config::load();
                                DaemonResponse::Config {
                                    watch_dirs: cfg.watch_dirs,
                                    collectors: cfg.collectors,
                                    privacy: cfg.privacy,
                                    ai: cfg.ai,
                                }
                            }
                            DaemonRequest::SetConfig {
                                watch_dirs,
                                collectors,
                                privacy,
                                ai,
                            } => {
                                let mut cfg = chronicle_config::load();
                                cfg.watch_dirs = watch_dirs;
                                cfg.collectors = collectors;
                                cfg.privacy = privacy;
                                cfg.ai = ai;
                                match chronicle_config::save(&cfg) {
                                    Ok(()) => DaemonResponse::Ack {
                                        event_id: "config_saved".into(),
                                    },
                                    Err(e) => DaemonResponse::Error {
                                        code: 500,
                                        message: format!("save config failed: {e}"),
                                    },
                                }
                            }
                            DaemonRequest::InstallShellHook { shell } => {
                                match chronicle_hooks::install(shell.as_deref()) {
                                    Ok(()) => DaemonResponse::Ack {
                                        event_id: "hook_installed".into(),
                                    },
                                    Err(e) => DaemonResponse::Error {
                                        code: 500,
                                        message: format!("hook install failed: {e}"),
                                    },
                                }
                            }
                            DaemonRequest::EmitEvent { event } => {
                                let event_id = event.id.to_string();
                                match event_tx.send(event).await {
                                    Ok(()) => DaemonResponse::Ack { event_id },
                                    Err(_) => DaemonResponse::Error {
                                        code: 503,
                                        message: "event pipeline unavailable".into(),
                                    },
                                }
                            }
                            _ => DaemonResponse::Error {
                                code: 400,
                                message: "unimplemented".into(),
                            },
                        }
                    }
                };
                if let Err(e) = conn.send_response(resp).await {
                    warn!("send response: {e}");
                }
            }
        },
        Err(e) => {
            warn!("read request: {e}");
            let _ = conn
                .send_response(DaemonResponse::Error {
                    code: 400,
                    message: format!("bad request: {e}"),
                })
                .await;
        }
    }
}
