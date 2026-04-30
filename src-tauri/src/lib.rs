mod app_state;
mod capture;
mod capture_guard;
mod commands;
#[cfg(debug_assertions)]
mod dev_bridge;
mod models;
mod recognition;
mod recorder;
mod resident;
mod storage;
mod summary;

use app_state::AppState;
use recognition::ensure_resident_worker;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = AppState::new()?;
            let config = state.storage.read_config().ok();
            #[cfg(debug_assertions)]
            dev_bridge::start(state.clone());
            app.manage(state);
            resident::setup(app)?;
            if let Some(config) = config {
                ensure_resident_worker(&config);
            }
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
            commands::get_today_summary_blocks,
            commands::summarize_previous_half_hour,
            commands::generate_summary,
            commands::generate_summary_report,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MindBack");
}
