use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

pub const DEFAULT_MODEL: &str = "mlx-community/Qwen3-VL-4B-Instruct-4bit";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub project_name: String,
    pub project_description: String,
    pub interval_seconds: u64,
    pub model: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            project_name: String::new(),
            project_description: String::new(),
            interval_seconds: 60,
            model: DEFAULT_MODEL.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecognitionResult {
    pub intent: String,
    pub is_on_project: bool,
    pub confidence: f32,
    pub reason: String,
    pub visible_context: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub project: String,
    pub screenshot_thumb: String,
    pub model: String,
    pub intent: String,
    pub is_on_project: bool,
    pub confidence: f32,
    pub reason: String,
    pub visible_context: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppStatus {
    pub is_recording: bool,
    pub today: String,
    pub project_name: String,
    pub last_error: Option<String>,
}
