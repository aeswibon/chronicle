use chronicle_core::{CanonicalEvent, ProjectRecord, Span};
use chronicle_ipc::{Client, DaemonRequest, DaemonResponse, SearchMode};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

use crate::DaemonState;
use crate::icons;

static EVENT_STREAM_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Serialize)]
pub struct StatusInfo {
    pub uptime_secs: u64,
    pub events_count: u64,
    pub version: String,
}

#[tauri::command]
pub async fn get_status(state: State<'_, DaemonState>) -> Result<StatusInfo, String> {
    let socket_path = state.socket_path.clone();
    let mut client = Client::connect(&socket_path).await?;

    match client.request(DaemonRequest::GetStatus).await? {
        DaemonResponse::Status {
            uptime_secs,
            events_count,
            version,
        } => Ok(StatusInfo {
            uptime_secs,
            events_count,
            version,
        }),
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
}

#[tauri::command]
pub async fn get_timeline(
    state: State<'_, DaemonState>,
    since: i64,
    until: Option<i64>,
    limit: u32,
) -> Result<Vec<Span>, String> {
    let socket_path = state.socket_path.clone();
    let mut client = Client::connect(&socket_path).await?;

    match client
        .request(DaemonRequest::GetTimeline { since, until, limit })
        .await?
    {
        DaemonResponse::Timeline { spans } => Ok(spans),
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
}

#[tauri::command]
pub async fn get_events(
    state: State<'_, DaemonState>,
    since: i64,
    until: Option<i64>,
    limit: u32,
) -> Result<Vec<CanonicalEvent>, String> {
    let socket_path = state.socket_path.clone();
    let mut client = Client::connect(&socket_path).await?;

    match client
        .request(DaemonRequest::GetEvents { since, until, limit })
        .await?
    {
        DaemonResponse::TimelineEvents { events } => Ok(events),
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
}

#[tauri::command]
pub async fn search_events(
    state: State<'_, DaemonState>,
    query: String,
    limit: u32,
    semantic: Option<bool>,
) -> Result<Vec<CanonicalEvent>, String> {
    let socket_path = state.socket_path.clone();
    let mut client = Client::connect(&socket_path).await?;
    let mode = if semantic.unwrap_or(false) {
        SearchMode::Semantic
    } else {
        SearchMode::Keyword
    };

    match client
        .request(DaemonRequest::Search {
            query,
            mode,
            limit,
        })
        .await?
    {
        DaemonResponse::TimelineEvents { events } => Ok(events),
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
}

#[tauri::command]
pub async fn get_errors(
    state: State<'_, DaemonState>,
    since: i64,
    limit: u32,
) -> Result<Vec<CanonicalEvent>, String> {
    let socket_path = state.socket_path.clone();
    let mut client = Client::connect(&socket_path).await?;
    match client
        .request(DaemonRequest::GetErrors { since, limit })
        .await?
    {
        DaemonResponse::TimelineEvents { events } => Ok(events),
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
}

#[tauri::command]
pub async fn get_sessions(
    state: State<'_, DaemonState>,
    since: i64,
    until: Option<i64>,
) -> Result<Vec<chronicle_core::Session>, String> {
    let socket_path = state.socket_path.clone();
    let mut client = Client::connect(&socket_path).await?;
    match client
        .request(DaemonRequest::GetSessions { since, until })
        .await?
    {
        DaemonResponse::Sessions { sessions } => Ok(sessions),
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
}

#[derive(Serialize)]
pub struct SummarizeResult {
    pub summary: String,
    pub persisted: bool,
    pub source: String,
    pub notice: Option<String>,
}

fn summarize_needs_fallback(message: &str) -> bool {
    message.contains("unknown variant `summarize_day`") || message.contains("unimplemented")
}

async fn summarize_day_fallback(
    client: &mut Client,
    since: i64,
    until: i64,
) -> Result<SummarizeResult, String> {
    let spans = match client
        .request(DaemonRequest::GetTimeline {
            since,
            until: Some(until),
            limit: 200,
        })
        .await?
    {
        DaemonResponse::Timeline { spans } => spans,
        DaemonResponse::Error { message, .. } => return Err(message),
        _ => return Err("unexpected timeline response".into()),
    };

    let events = match client
        .request(DaemonRequest::GetEvents {
            since,
            until: Some(until),
            limit: 500,
        })
        .await?
    {
        DaemonResponse::TimelineEvents { events } => events,
        DaemonResponse::Error { message, .. } => return Err(message),
        _ => return Err("unexpected events response".into()),
    };

    let cfg = chronicle_config::load();
    let (summary, source) =
        chronicle_ai::summarize_day(&cfg.ai, since, until, &spans, &events).await;
    let source = match source {
        chronicle_ai::SummarySource::Ai => "ai",
        chronicle_ai::SummarySource::Rules => "rules",
    };
    Ok(SummarizeResult {
        summary,
        persisted: false,
        source: source.into(),
        notice: Some(
            "Summary generated locally. Rebuild and restart the daemon to persist rollups: make install-daemon".into(),
        ),
    })
}

#[tauri::command]
pub async fn summarize_day(
    state: State<'_, DaemonState>,
    since: i64,
    until: Option<i64>,
) -> Result<SummarizeResult, String> {
    let socket_path = state.socket_path.clone();
    let mut client = Client::connect(&socket_path).await?;
    let until = until.unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    match client
        .request(DaemonRequest::SummarizeDay {
            since,
            until: Some(until),
        })
        .await?
    {
        DaemonResponse::DailySummary {
            summary,
            source,
            ..
        } => Ok(SummarizeResult {
            summary,
            persisted: true,
            source: source.unwrap_or_else(|| "rules".into()),
            notice: None,
        }),
        DaemonResponse::Error { message, .. } if summarize_needs_fallback(&message) => {
            summarize_day_fallback(&mut client, since, until).await
        }
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
}

#[derive(Serialize)]
pub struct ProjectContextInfo {
    pub project: Option<ProjectRecord>,
    pub spans: Vec<Span>,
    pub events: Vec<CanonicalEvent>,
}

#[tauri::command]
pub async fn get_project_context(
    state: State<'_, DaemonState>,
    project: String,
    since: i64,
    limit: u32,
) -> Result<ProjectContextInfo, String> {
    let socket_path = state.socket_path.clone();
    let mut client = Client::connect(&socket_path).await?;
    match client
        .request(DaemonRequest::GetProjectContext {
            project,
            since,
            limit,
        })
        .await?
    {
        DaemonResponse::ProjectContext {
            project,
            spans,
            events,
        } => Ok(ProjectContextInfo {
            project,
            spans,
            events,
        }),
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
}

#[derive(Serialize)]
pub struct SpanDetailInfo {
    pub span: Span,
    pub events: Vec<CanonicalEvent>,
}

#[tauri::command]
pub async fn get_span_detail(
    state: State<'_, DaemonState>,
    id: String,
    event_limit: u32,
) -> Result<SpanDetailInfo, String> {
    let socket_path = state.socket_path.clone();
    let mut client = Client::connect(&socket_path).await?;
    match client
        .request(DaemonRequest::GetSpan {
            id,
            event_limit,
        })
        .await?
    {
        DaemonResponse::SpanDetail { span, events } => Ok(SpanDetailInfo { span, events }),
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
}

#[derive(Serialize, Deserialize)]
pub struct CollectorsInfo {
    pub window_focus: bool,
    pub filesystem: bool,
    pub git: bool,
    pub shell: bool,
}

#[derive(Serialize, Deserialize)]
pub struct PrivacyInfo {
    pub allowed_domains: Vec<String>,
    pub strip_query_params: bool,
    pub retention_days: Option<u32>,
    pub redact_shell_secrets: bool,
}

#[derive(Serialize, Deserialize)]
pub struct AiInfo {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub api_key_env: Option<String>,
    pub timeout_secs: u64,
}

#[derive(Serialize, Deserialize)]
pub struct ConfigInfo {
    pub watch_dirs: Vec<String>,
    pub collectors: CollectorsInfo,
    pub privacy: PrivacyInfo,
    pub ai: AiInfo,
}

impl From<chronicle_config::CollectorsConfig> for CollectorsInfo {
    fn from(c: chronicle_config::CollectorsConfig) -> Self {
        Self {
            window_focus: c.window_focus,
            filesystem: c.filesystem,
            git: c.git,
            shell: c.shell,
        }
    }
}

impl From<CollectorsInfo> for chronicle_config::CollectorsConfig {
    fn from(c: CollectorsInfo) -> Self {
        Self {
            window_focus: c.window_focus,
            filesystem: c.filesystem,
            git: c.git,
            shell: c.shell,
        }
    }
}

impl From<chronicle_config::PrivacyConfig> for PrivacyInfo {
    fn from(p: chronicle_config::PrivacyConfig) -> Self {
        Self {
            allowed_domains: p.allowed_domains,
            strip_query_params: p.strip_query_params,
            retention_days: p.retention_days,
            redact_shell_secrets: p.redact_shell_secrets,
        }
    }
}

impl From<PrivacyInfo> for chronicle_config::PrivacyConfig {
    fn from(p: PrivacyInfo) -> Self {
        Self {
            allowed_domains: p.allowed_domains,
            strip_query_params: p.strip_query_params,
            retention_days: p.retention_days,
            redact_shell_secrets: p.redact_shell_secrets,
        }
    }
}

impl From<chronicle_config::AiConfig> for AiInfo {
    fn from(a: chronicle_config::AiConfig) -> Self {
        Self {
            enabled: a.enabled,
            base_url: a.base_url,
            model: a.model,
            api_key_env: a.api_key_env,
            timeout_secs: a.timeout_secs,
        }
    }
}

impl From<AiInfo> for chronicle_config::AiConfig {
    fn from(a: AiInfo) -> Self {
        Self {
            enabled: a.enabled,
            base_url: a.base_url,
            model: a.model,
            api_key_env: a.api_key_env,
            timeout_secs: a.timeout_secs,
        }
    }
}

#[tauri::command]
pub async fn get_config(_state: State<'_, DaemonState>) -> Result<ConfigInfo, String> {
    tokio::task::spawn_blocking(|| {
        let cfg = chronicle_config::load();
        ConfigInfo {
            watch_dirs: cfg.watch_dirs,
            collectors: cfg.collectors.into(),
            privacy: cfg.privacy.into(),
            ai: cfg.ai.into(),
        }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_config(
    _state: State<'_, DaemonState>,
    watch_dirs: Vec<String>,
    collectors: CollectorsInfo,
    privacy: PrivacyInfo,
    ai: AiInfo,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let cfg = chronicle_config::ChronicleConfig {
            watch_dirs,
            collectors: collectors.into(),
            privacy: privacy.into(),
            ai: ai.into(),
        };
        chronicle_config::save(&cfg).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn install_shell_hook(
    _state: State<'_, DaemonState>,
    shell: Option<String>,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        chronicle_hooks::install(shell.as_deref()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn restart_daemon() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        #[cfg(target_os = "macos")]
        {
            let release = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../target/release/chronicle-daemon");
            if release.is_file() {
                let _ = std::process::Command::new(&release)
                    .arg("install")
                    .status();
            }
            let uid = std::process::Command::new("id")
                .arg("-u")
                .output()
                .map_err(|e| e.to_string())?;
            let uid = String::from_utf8_lossy(&uid.stdout).trim().to_string();
            let label = format!("gui/{uid}/com.chronicle.daemon");
            let status = std::process::Command::new("launchctl")
                .args(["kickstart", "-k", &label])
                .status()
                .map_err(|e| e.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err(
                    "launchctl kickstart failed — install the daemon with chronicle-daemon install"
                        .into(),
                )
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = std::process::Command::new("pkill")
                .args(["-f", "chronicle-daemon"])
                .status();
            Err("daemon restart is only supported on macOS via launchctl".into())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn list_projects(
    state: State<'_, DaemonState>,
    limit: u32,
) -> Result<Vec<ProjectRecord>, String> {
    let socket_path = state.socket_path.clone();
    tokio::time::timeout(Duration::from_secs(5), fetch_projects(&socket_path, limit))
        .await
        .map_err(|_| "daemon request timed out — try restarting chronicle-daemon".to_string())?
}

async fn fetch_projects(socket_path: &str, limit: u32) -> Result<Vec<ProjectRecord>, String> {
    let mut client = Client::connect(socket_path).await?;
    match client
        .request(DaemonRequest::ListProjects { limit })
        .await?
    {
        DaemonResponse::Projects { projects } => Ok(projects),
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
}

#[tauri::command]
pub async fn resolve_app_icon(
    bundle_id: Option<String>,
    app_name: Option<String>,
) -> Result<Option<String>, String> {
    Ok(icons::resolve_app_icon(bundle_id, app_name))
}

#[tauri::command]
pub async fn resolve_path_icons(
    paths: Vec<String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let paths = paths.clone();
    tokio::task::spawn_blocking(move || icons::resolve_path_icons(paths))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_event_stream(app: AppHandle, state: State<'_, DaemonState>) -> Result<(), String> {
    if EVENT_STREAM_ACTIVE.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let socket_path = state.socket_path.clone();
    tokio::spawn(async move {
        loop {
            match run_event_stream(&app, &socket_path).await {
                Ok(()) => break,
                Err(err) => {
                    let _ = app.emit("chronicle-stream-error", err);
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
        EVENT_STREAM_ACTIVE.store(false, Ordering::SeqCst);
    });

    Ok(())
}

async fn run_event_stream(app: &AppHandle, socket_path: &str) -> Result<(), String> {
    let client = Client::connect(socket_path).await?;
    let mut stream = client.subscribe(vec![]).await?;

    loop {
        match stream.next().await? {
            DaemonResponse::Event { event } => {
                app.emit("chronicle-event", &event)
                    .map_err(|e| e.to_string())?;
            }
            DaemonResponse::Error { message, .. } => return Err(message),
            _ => {}
        }
    }
}
