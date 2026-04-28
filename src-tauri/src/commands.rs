use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use chrono::Local;
use tauri::State;

use crate::{
    app_state::AppState,
    models::{AppConfig, AppStatus, LogEntry},
    recorder,
    storage::Storage,
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
    state.recording.store(true, Ordering::SeqCst);
    ensure_recording_worker(
        state.storage.clone(),
        Arc::clone(&state.recording),
        Arc::clone(&state.worker_running),
        Arc::clone(&state.last_error),
    );
    get_status(state)
}

#[tauri::command]
pub fn stop_recording(state: State<'_, AppState>) -> Result<AppStatus, String> {
    state.recording.store(false, Ordering::SeqCst);
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
pub fn generate_summary(state: State<'_, AppState>) -> Result<String, String> {
    let path = state
        .storage
        .write_today_summary()
        .map_err(to_command_error)?;
    Ok(path.display().to_string())
}

fn ensure_recording_worker(
    storage: Storage,
    recording: Arc<AtomicBool>,
    worker_running: Arc<AtomicBool>,
    last_error: Arc<Mutex<Option<String>>>,
) {
    if worker_running.swap(true, Ordering::SeqCst) {
        return;
    }

    thread::spawn(move || {
        while recording.load(Ordering::SeqCst) {
            let interval_seconds = match storage.read_config() {
                Ok(config) => {
                    let interval = config.interval_seconds.clamp(10, 3600);
                    if let Err(error) = recorder::record_once(&storage, &config) {
                        if let Ok(mut last_error) = last_error.lock() {
                            *last_error = Some(error.to_string());
                        }
                    }
                    interval
                }
                Err(error) => {
                    if let Ok(mut last_error) = last_error.lock() {
                        *last_error = Some(error.to_string());
                    }
                    60
                }
            };

            for _ in 0..interval_seconds {
                if !recording.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_secs(1));
            }
        }

        worker_running.store(false, Ordering::SeqCst);
    });
}
