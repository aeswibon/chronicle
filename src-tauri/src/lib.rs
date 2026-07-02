pub mod commands;
pub mod daemon_manage;
pub mod focus_relay;
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
            socket_path: daemon_manage::default_socket_string(),
            client: Mutex::new(None),
        })
        .setup(|_app| {
            std::thread::spawn(|| {
                if let Err(e) = daemon_manage::ensure_daemon_running() {
                    eprintln!("chronicle ensure_daemon: {e}");
                }
            });
            // Window focus is captured by chronicle-daemon (launchctl asuser helper).
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                window.app_handle().exit(0);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::request_macos_accessibility,
            commands::open_macos_privacy_settings,
            commands::get_timeline,
            commands::get_events,
            commands::search_events,
            commands::get_errors,
            commands::get_sessions,
            commands::delete_session,
            commands::summarize_day,
            commands::summarize_today,
            commands::get_project_context,
            commands::get_span_detail,
            commands::get_config,
            commands::set_config,
            commands::test_ai_connection,
            commands::list_ollama_models,
            commands::install_shell_hook,
            commands::restart_daemon,
            commands::prune_noise_events,
            commands::purge_capture_timeline,
            commands::ensure_daemon,
            commands::list_projects,
            commands::start_event_stream,
            commands::resolve_app_icon,
            commands::resolve_path_icons,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
