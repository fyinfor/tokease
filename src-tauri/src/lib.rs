//! Tauri shell: wires `tokease_core::Tokease` to IPC commands. No business
//! logic lives here.

mod commands;

use std::sync::Arc;

use tokease_core::Tokease;

pub fn run() {
    let core = Tokease::new().expect("initialise Tokease data directory");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: Some("tokease".into()) }),
                ])
                .build(),
        )
        .manage(Arc::new(core))
        .invoke_handler(tauri::generate_handler![
            commands::get_session,
            commands::set_server_url,
            commands::start_device_login,
            commands::poll_device_login,
            commands::login_with_password,
            commands::logout,
            commands::list_clients,
            commands::enable_client,
            commands::restore_client,
            commands::list_backups,
            commands::get_platform_config,
            commands::refresh_platform_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tokease");
}
