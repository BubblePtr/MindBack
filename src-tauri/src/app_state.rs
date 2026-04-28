use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use crate::{recorder, storage::Storage};

#[derive(Clone)]
pub struct AppState {
    pub storage: Storage,
    pub recording: Arc<AtomicBool>,
    pub worker_running: Arc<AtomicBool>,
    pub last_error: Arc<Mutex<Option<String>>>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self::new_with_storage(Storage::default_location()?))
    }

    pub fn new_with_storage(storage: Storage) -> Self {
        Self {
            storage,
            recording: Arc::new(AtomicBool::new(false)),
            worker_running: Arc::new(AtomicBool::new(false)),
            last_error: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start_recording_worker(&self) {
        self.recording.store(true, Ordering::SeqCst);
        ensure_recording_worker(
            self.storage.clone(),
            Arc::clone(&self.recording),
            Arc::clone(&self.worker_running),
            Arc::clone(&self.last_error),
        );
    }

    pub fn stop_recording_worker(&self) {
        self.recording.store(false, Ordering::SeqCst);
    }
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use tempfile::tempdir;

    use super::AppState;
    use crate::storage::Storage;

    #[test]
    fn recording_lifecycle_flips_recording_state() {
        std::env::set_var("MINDBACK_SIMULATE_CAPTURE", "1");
        let dir = tempdir().unwrap();
        let state = AppState::new_with_storage(Storage::new(dir.path()).unwrap());

        assert!(!state.recording.load(Ordering::SeqCst));

        state.start_recording_worker();
        assert!(state.recording.load(Ordering::SeqCst));

        state.stop_recording_worker();
        assert!(!state.recording.load(Ordering::SeqCst));
    }
}
