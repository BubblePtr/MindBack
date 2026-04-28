use std::sync::{atomic::AtomicBool, Arc, Mutex};

use crate::storage::Storage;

pub struct AppState {
    pub storage: Storage,
    pub recording: Arc<AtomicBool>,
    pub worker_running: Arc<AtomicBool>,
    pub last_error: Arc<Mutex<Option<String>>>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            storage: Storage::default_location()?,
            recording: Arc::new(AtomicBool::new(false)),
            worker_running: Arc::new(AtomicBool::new(false)),
            last_error: Arc::new(Mutex::new(None)),
        })
    }
}
