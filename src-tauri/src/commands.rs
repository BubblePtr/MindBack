use std::sync::atomic::Ordering;

use base64::{engine::general_purpose, Engine as _};
use chrono::Local;
use tauri::State;

use crate::{
    app_state::AppState,
    models::{AppConfig, AppStatus, LogEntry},
    recorder,
    summary::SummaryService,
};

fn to_command_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    state.storage.read_config().map_err(to_command_error)
}

#[tauri::command]
pub fn save_config(config: AppConfig, state: State<'_, AppState>) -> Result<AppConfig, String> {
    state
        .storage
        .save_config(&config)
        .map_err(to_command_error)?;
    Ok(config)
}

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> Result<AppStatus, String> {
    let config = state.storage.read_config().map_err(to_command_error)?;
    let last_error = state.last_error.lock().map_err(to_command_error)?.clone();
    Ok(AppStatus {
        is_recording: state.recording.load(Ordering::SeqCst),
        today: Local::now().date_naive().format("%Y-%m-%d").to_string(),
        project_name: config.project_name,
        last_error,
    })
}

#[tauri::command]
pub fn start_recording(state: State<'_, AppState>) -> Result<AppStatus, String> {
    state.start_recording_worker();
    get_status(state)
}

#[tauri::command]
pub fn stop_recording(state: State<'_, AppState>) -> Result<AppStatus, String> {
    state.stop_recording_worker();
    get_status(state)
}

#[tauri::command]
pub fn record_once(state: State<'_, AppState>) -> Result<LogEntry, String> {
    let config = state.storage.read_config().map_err(to_command_error)?;
    recorder::record_once(&state.storage, &config).map_err(|error| {
        if let Ok(mut last_error) = state.last_error.lock() {
            *last_error = Some(error.to_string());
        }
        error.to_string()
    })
}

#[tauri::command]
pub fn list_today_entries(state: State<'_, AppState>) -> Result<Vec<LogEntry>, String> {
    state.storage.list_today_entries().map_err(to_command_error)
}

#[tauri::command]
pub fn get_today_thumbnail(
    screenshot_thumb: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let bytes = state
        .storage
        .read_today_thumb(&screenshot_thumb)
        .map_err(to_command_error)?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        general_purpose::STANDARD.encode(bytes)
    ))
}

#[tauri::command]
pub fn generate_summary(state: State<'_, AppState>) -> Result<String, String> {
    let path = state
        .storage
        .write_today_summary()
        .map_err(to_command_error)?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn get_today_summary_blocks(
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::SummaryTimeBlock>, String> {
    SummaryService::new(&state.storage)
        .today_summary_blocks()
        .map_err(to_command_error)
}

#[tauri::command]
pub fn summarize_previous_half_hour(
    state: State<'_, AppState>,
) -> Result<Option<crate::models::SummaryTimeBlock>, String> {
    let config = state.storage.read_config().map_err(to_command_error)?;
    SummaryService::new(&state.storage)
        .summarize_previous_half_hour(&config)
        .map_err(|error| {
            if let Ok(mut last_error) = state.last_error.lock() {
                *last_error = Some(error.to_string());
            }
            error.to_string()
        })
}
