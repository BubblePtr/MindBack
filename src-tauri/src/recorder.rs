use anyhow::Result;
use chrono::Local;

use crate::{
    capture::CaptureService,
    models::{AppConfig, LogEntry},
    recognition::RecognitionService,
    storage::Storage,
};

pub fn record_once(storage: &Storage, config: &AppConfig) -> Result<LogEntry> {
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
        error: None,
    };

    storage.append_log_entry(&entry)?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::record_once;
    use crate::{models::AppConfig, storage::Storage};

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
}
