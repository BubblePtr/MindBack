use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

pub const DEFAULT_MODEL: &str = "mlx-community/Qwen3-VL-4B-Instruct-4bit";
pub const DEFAULT_SUMMARY_MODEL: &str = "deepseek-chat";

fn default_interval_seconds() -> u64 {
    60
}

fn default_summary_model() -> String {
    DEFAULT_SUMMARY_MODEL.to_string()
}

fn default_summary_provider() -> String {
    "deepseek".to_string()
}

fn default_summary_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    #[serde(default)]
    pub project_name: String,
    #[serde(default)]
    pub project_description: String,
    #[serde(default = "default_interval_seconds")]
    pub interval_seconds: u64,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_summary_model")]
    pub summary_model: String,
    #[serde(default = "default_summary_provider")]
    pub summary_provider: String,
    #[serde(default = "default_summary_enabled")]
    pub summary_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            project_name: String::new(),
            project_description: String::new(),
            interval_seconds: 60,
            model: DEFAULT_MODEL.to_string(),
            summary_model: DEFAULT_SUMMARY_MODEL.to_string(),
            summary_provider: "deepseek".to_string(),
            summary_enabled: true,
        }
    }
}

fn default_model() -> String {
    DEFAULT_MODEL.to_string()
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SummaryBlockStatus {
    OnProject,
    OffProject,
    Uncertain,
    InsufficientData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SummaryAssessment {
    Focused,
    Mixed,
    Drifted,
    InsufficientData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAlignment {
    pub on_project_ratio: u8,
    pub assessment: SummaryAssessment,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryTimeBlock {
    pub start: String,
    pub end: String,
    pub status: SummaryBlockStatus,
    pub summary: String,
    pub evidence: Vec<String>,
    #[serde(default)]
    pub record_count: usize,
    #[serde(default)]
    pub on_project_ratio: u8,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NotableDrift {
    pub time: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryAgentResult {
    pub overview: String,
    pub project_alignment: ProjectAlignment,
    pub time_blocks: Vec<SummaryTimeBlock>,
    pub notable_drifts: Vec<NotableDrift>,
    pub reflection_prompts: Vec<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryLogEntry {
    pub timestamp: String,
    pub intent: String,
    pub is_on_project: bool,
    pub confidence: f32,
    pub reason: String,
    pub visible_context: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryAgentRequest {
    pub task: String,
    pub date: String,
    pub project: String,
    pub window_start: Option<String>,
    pub window_end: Option<String>,
    pub entries: Vec<SummaryLogEntry>,
    pub time_blocks: Vec<SummaryTimeBlock>,
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn default_summary_provider_is_deepseek_for_mvp() {
        let config = AppConfig::default();

        assert_eq!(config.summary_provider, "deepseek");
        assert_eq!(config.summary_model, "deepseek-chat");
    }
}
