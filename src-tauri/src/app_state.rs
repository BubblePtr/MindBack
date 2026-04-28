use std::sync::Mutex;

use crate::storage::Storage;

pub struct AppState {
    pub storage: Storage,
    pub recording: Mutex<bool>,
    pub last_error: Mutex<Option<String>>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            storage: Storage::default_location()?,
            recording: Mutex::new(false),
            last_error: Mutex::new(None),
        })
    }
}
