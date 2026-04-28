use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::models::{AppConfig, RecognitionResult};

pub struct RecognitionService;

impl RecognitionService {
    pub fn recognize(image_path: &Path, config: &AppConfig) -> RecognitionResult {
        Self::recognize_with_worker(image_path, config)
    }

    fn recognize_with_worker(image_path: &Path, config: &AppConfig) -> RecognitionResult {
        let python = std::env::var("MINDBACK_MLX_PYTHON")
            .unwrap_or_else(|_| default_mlx_python_command().display().to_string());
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
}

fn default_mlx_python_command() -> PathBuf {
    let app_venv_python = default_mlx_python_path();
    if app_venv_python.exists() {
        return app_venv_python;
    }

    PathBuf::from("python3")
}

fn default_mlx_python_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("MindBack")
        .join("venvs")
        .join("mlx-worker")
        .join("bin")
        .join("python")
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

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::models::AppConfig;

    use super::default_mlx_python_path;
    use super::RecognitionService;

    #[test]
    fn default_worker_python_path_is_persistent_app_data() {
        let path = default_mlx_python_path();

        assert!(path.to_string_lossy().contains("MindBack/venvs/mlx-worker"));
        assert!(!path.starts_with("/tmp"));
    }

    #[test]
    fn recognize_uses_worker_by_default() {
        std::env::remove_var("MINDBACK_ENABLE_MLX");
        let dir = tempdir().unwrap();
        let worker_path = dir.path().join("fake_worker.sh");
        let image_path = dir.path().join("capture.jpg");
        fs::write(&image_path, "fake image").unwrap();
        fs::write(
            &worker_path,
            "echo '{\"intent\":\"正在查看项目文档\",\"is_on_project\":true,\"confidence\":0.91,\"reason\":\"截图内容与今日项目相关\",\"visible_context\":\"屏幕中显示项目文档\",\"error\":null}'\n",
        )
        .unwrap();
        std::env::set_var("MINDBACK_MLX_PYTHON", "/bin/sh");
        std::env::set_var("MINDBACK_WORKER_PATH", &worker_path);

        let result = RecognitionService::recognize(&image_path, &AppConfig::default());

        std::env::remove_var("MINDBACK_MLX_PYTHON");
        std::env::remove_var("MINDBACK_WORKER_PATH");

        assert_eq!(result.intent, "正在查看项目文档");
        assert_eq!(result.visible_context, "屏幕中显示项目文档");
        assert_eq!(result.error, None);
    }
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
