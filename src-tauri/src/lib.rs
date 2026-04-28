mod app_state;
mod capture;
mod commands;
mod models;
mod recognition;
mod recorder;
mod resident;
mod storage;

use app_state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = AppState::new()?;
            app.manage(state);
            resident::setup(app)?;
            Ok(())
        })
        .on_window_event(resident::handle_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::get_status,
            commands::start_recording,
            commands::stop_recording,
            commands::record_once,
            commands::list_today_entries,
            commands::get_today_thumbnail,
            commands::generate_summary,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MindBack");
}
