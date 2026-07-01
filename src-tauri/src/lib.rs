pub mod commands;
pub mod icons;

use std::sync::Mutex;
use chronicle_ipc::Client;
use tauri::Manager;

pub struct DaemonState {
    pub socket_path: String,
    pub client: Mutex<Option<Client>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(DaemonState {
            socket_path: "/tmp/chronicle.sock".into(),
            client: Mutex::new(None),
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                window.app_handle().exit(0);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_timeline,
            commands::get_events,
            commands::search_events,
            commands::get_errors,
            commands::get_project_context,
            commands::get_span_detail,
            commands::get_config,
            commands::set_config,
            commands::install_shell_hook,
            commands::restart_daemon,
            commands::list_projects,
            commands::start_event_stream,
            commands::resolve_app_icon,
            commands::resolve_path_icons,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
