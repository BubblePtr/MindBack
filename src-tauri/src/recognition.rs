use std::path::Path;

use crate::models::{AppConfig, RecognitionResult};

pub struct RecognitionService;

impl RecognitionService {
    pub fn recognize(_image_path: &Path, config: &AppConfig) -> RecognitionResult {
        let project = if config.project_name.trim().is_empty() {
            "今日项目".to_string()
        } else {
            config.project_name.clone()
        };

        RecognitionResult {
            intent: format!("正在推进 {project}"),
            is_on_project: true,
            confidence: 0.8,
            reason: "当前 MVP 使用本地识别接口的确定性结果，真实 MLX-VLM 识别将在 worker 接入后替换。"
                .to_string(),
            visible_context: "MindBack 本地记录闭环 smoke path".to_string(),
        }
    }
}
