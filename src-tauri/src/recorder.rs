use anyhow::Result;
use chrono::Local;

use crate::{
    capture::CaptureService,
    capture_guard::CaptureAvailability,
    models::{AppConfig, LogEntry},
    recognition::RecognitionService,
    storage::Storage,
};

pub fn record_once(storage: &Storage, config: &AppConfig) -> Result<LogEntry> {
    CaptureAvailability::current().ensure_allowed()?;
    record_once_unchecked(storage, config)
}

pub fn record_once_if_allowed(
    storage: &Storage,
    config: &AppConfig,
    availability: CaptureAvailability,
) -> Result<Option<LogEntry>> {
    if !availability.is_allowed() {
        return Ok(None);
    }

    record_once_unchecked(storage, config).map(Some)
}

pub fn record_once_if_display_available(
    storage: &Storage,
    config: &AppConfig,
) -> Result<Option<LogEntry>> {
    record_once_if_allowed(storage, config, CaptureAvailability::current())
}

fn record_once_unchecked(storage: &Storage, config: &AppConfig) -> Result<LogEntry> {
    let day_dir = storage.today_dir()?;
    let capture = CaptureService::capture_once(day_dir)?;
    let recognition = RecognitionService::recognize(&capture.image_path, config);
    let entry = LogEntry {
        timestamp: Local::now(),
        project: config.project_name.clone(),
        screenshot_thumb: capture.thumb_relative_path,
        model: config.model.clone(),
        intent: recognition.intent,
        is_on_project: recognition.is_on_project,
        confidence: recognition.confidence,
        reason: recognition.reason,
        visible_context: recognition.visible_context,
        error: recognition.error,
    };

    storage.append_log_entry(&entry)?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{record_once, record_once_if_allowed};
    use crate::{
        capture_guard::{CaptureAvailability, DisplayState},
        models::AppConfig,
        storage::Storage,
    };

    #[test]
    fn record_once_writes_entry_and_thumbnail() {
        std::env::set_var("MINDBACK_SIMULATE_CAPTURE", "1");
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path()).unwrap();
        let config = AppConfig {
            project_name: "MindBack MVP".to_string(),
            ..AppConfig::default()
        };

        let entry = record_once(&storage, &config).unwrap();
        let entries = storage.list_today_entries().unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].intent, entry.intent);
        assert!(storage
            .today_dir()
            .unwrap()
            .join(entry.screenshot_thumb)
            .exists());
    }

    #[test]
    fn automatic_recording_skips_when_display_is_asleep() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path()).unwrap();
        let config = AppConfig {
            project_name: "MindBack MVP".to_string(),
            ..AppConfig::default()
        };
        let availability = CaptureAvailability::from_display_state(DisplayState {
            is_online: true,
            is_active: false,
            is_asleep: true,
        });

        let entry = record_once_if_allowed(&storage, &config, availability).unwrap();

        assert!(entry.is_none());
        assert!(storage.list_today_entries().unwrap().is_empty());
    }
}
