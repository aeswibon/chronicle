use chronicle_ipc::{Client, DaemonRequest, DaemonResponse};
use serde::Serialize;
use std::sync::Mutex;
use tauri::State;

struct DaemonState {
    socket_path: String,
    client: Mutex<Option<Client>>,
}

#[derive(Serialize)]
struct StatusResponse {
    connected: bool,
    uptime_secs: u64,
    events_count: u64,
    version: String,
}

#[tauri::command]
async fn get_status(state: State<'_, DaemonState>) -> Result<StatusResponse, String> {
    let mut client = Client::connect(&state.socket_path)
        .await
        .map_err(|e| format!("daemon not reachable: {e}"))?;

    let resp = client
        .request(DaemonRequest::GetStatus)
        .await
        .map_err(|e| e.to_string())?;

    match resp {
        DaemonResponse::Status {
            uptime_secs,
            events_count,
            version,
            ..
        } => Ok(StatusResponse {
            connected: true,
            uptime_secs,
            events_count,
            version,
        }),
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
}

#[tauri::command]
async fn get_timeline(
    state: State<'_, DaemonState>,
    since: i64,
) -> Result<Vec<chronicle_core::CanonicalEvent>, String> {
    let mut client = Client::connect(&state.socket_path)
        .await
        .map_err(|e| format!("daemon not reachable: {e}"))?;

    let resp = client
        .request(DaemonRequest::GetTimeline {
            since,
            until: None,
            limit: 50,
        })
        .await
        .map_err(|e| e.to_string())?;

    match resp {
        DaemonResponse::Timeline { spans } => Ok(spans),
        DaemonResponse::Error { message, .. } => Err(message),
        _ => Err("unexpected response".into()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(DaemonState {
            socket_path: "/tmp/chronicle.sock".into(),
            client: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![get_status, get_timeline])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
