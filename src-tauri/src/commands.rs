use chronicle_core::{CanonicalEvent, ProjectRecord, Span};
use chronicle_ipc::{Client, DaemonRequest, DaemonResponse, SearchMode};
use serde::Serialize;
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
) -> Result<Vec<CanonicalEvent>, String> {
    let socket_path = state.socket_path.clone();
    let mut client = Client::connect(&socket_path).await?;

    match client
        .request(DaemonRequest::Search {
            query,
            mode: SearchMode::Keyword,
            limit,
        })
        .await?
    {
        DaemonResponse::TimelineEvents { events } => Ok(events),
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

#[derive(Serialize)]
pub struct ConfigInfo {
    pub watch_dirs: Vec<String>,
}

#[tauri::command]
pub async fn get_config(state: State<'_, DaemonState>) -> Result<ConfigInfo, String> {
    let socket_path = state.socket_path.clone();
    let mut client = Client::connect(&socket_path).await?;
    match client.request(DaemonRequest::GetConfig).await? {
        DaemonResponse::Config { watch_dirs } => Ok(ConfigInfo { watch_dirs }),
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
}

#[tauri::command]
pub async fn set_config(
    state: State<'_, DaemonState>,
    watch_dirs: Vec<String>,
) -> Result<(), String> {
    let socket_path = state.socket_path.clone();
    let mut client = Client::connect(&socket_path).await?;
    match client
        .request(DaemonRequest::SetConfig { watch_dirs })
        .await?
    {
        DaemonResponse::Ack { .. } => Ok(()),
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
}

#[tauri::command]
pub async fn install_shell_hook(
    state: State<'_, DaemonState>,
    shell: Option<String>,
) -> Result<(), String> {
    let socket_path = state.socket_path.clone();
    let mut client = Client::connect(&socket_path).await?;
    match client
        .request(DaemonRequest::InstallShellHook { shell })
        .await?
    {
        DaemonResponse::Ack { .. } => Ok(()),
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
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
