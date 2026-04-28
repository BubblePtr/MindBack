use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::models::{AppConfig, RecognitionResult};

pub struct RecognitionService;

impl RecognitionService {
    pub fn recognize(image_path: &Path, config: &AppConfig) -> RecognitionResult {
        if std::env::var("MINDBACK_ENABLE_MLX").as_deref() == Ok("1") {
            return Self::recognize_with_worker(image_path, config);
        }

        Self::placeholder_result(config)
    }

    fn recognize_with_worker(image_path: &Path, config: &AppConfig) -> RecognitionResult {
        let python = std::env::var("MINDBACK_MLX_PYTHON")
            .unwrap_or_else(|_| "/tmp/mindback-mlx-venv/bin/python".to_string());
        let worker_path = std::env::var("MINDBACK_WORKER_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("workers/mlx_worker.py")
            });
        let model_path = resolve_model_path(&config.model);
        let project = if config.project_name.trim().is_empty() {
            "今日项目"
        } else {
            config.project_name.as_str()
        };

        let output = Command::new(&python)
            .arg(&worker_path)
            .arg("--model")
            .arg(&model_path)
            .arg("--project")
            .arg(project)
            .arg("--image")
            .arg(image_path)
            .output();

        let output = match output {
            Ok(output) => output,
            Err(error) => {
                return worker_error(format!("failed to launch MLX worker: {error}"));
            }
        };

        if !output.status.success() {
            return worker_error(format!(
                "MLX worker exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        match serde_json::from_slice::<RecognitionResult>(&output.stdout) {
            Ok(result) => result,
            Err(error) => worker_error(format!(
                "failed to parse MLX worker output: {error}; stdout: {}; stderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )),
        }
    }

    fn placeholder_result(config: &AppConfig) -> RecognitionResult {
        let project = if config.project_name.trim().is_empty() {
            "今日项目".to_string()
        } else {
            config.project_name.clone()
        };

        RecognitionResult {
            intent: format!("正在推进 {project}"),
            is_on_project: true,
            confidence: 0.8,
            reason:
                "当前 MVP 使用本地识别接口的确定性结果，真实 MLX-VLM 识别将在 worker 接入后替换。"
                    .to_string(),
            visible_context: "MindBack 本地记录闭环 smoke path".to_string(),
            error: Some("mlx_worker_disabled".to_string()),
        }
    }
}

fn resolve_model_path(model: &str) -> String {
    if let Ok(path) = std::env::var("MINDBACK_MLX_MODEL_PATH") {
        return path;
    }

    let configured_path = Path::new(model);
    if configured_path.exists() {
        return configured_path.display().to_string();
    }

    let local_model = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("MindBack")
        .join("models")
        .join(model);
    if local_model.exists() {
        return local_model.display().to_string();
    }

    model.to_string()
}

fn worker_error(error: String) -> RecognitionResult {
    RecognitionResult {
        intent: "未能完成本地视觉识别".to_string(),
        is_on_project: false,
        confidence: 0.0,
        reason: error.clone(),
        visible_context: String::new(),
        error: Some(error),
    }
}
