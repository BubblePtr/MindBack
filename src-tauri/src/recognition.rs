use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
};

use anyhow::Context;

use crate::models::{AppConfig, RecognitionResult};

// ─────────────────────────────────────────────────────────────
// Resident Worker
// ─────────────────────────────────────────────────────────────

/// A long-running MLX-VLM worker process that loads the model once
/// and answers recognition requests via stdin/stdout JSON Lines.
pub struct ResidentWorker {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    model: String,
}

impl ResidentWorker {
    pub fn new(config: &AppConfig) -> anyhow::Result<Self> {
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

        let mut child = Command::new(&python)
            .arg(&worker_path)
            .arg("--daemon")
            .arg("--model")
            .arg(&model_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| {
                format!("failed to launch MLX resident worker (python={python})")
            })?;

        let stdin = child
            .stdin
            .take()
            .context("failed to open worker stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("failed to open worker stdout")?;
        let mut stdout_reader = BufReader::new(stdout);

        let mut line = String::new();
        stdout_reader
            .read_line(&mut line)
            .context("failed to read worker ready signal")?;
        let ready: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("failed to parse worker ready signal: {line}"))?;
        if ready.get("__ready__").and_then(|v| v.as_bool()) != Some(true) {
            anyhow::bail!("worker did not send ready signal: {}", line.trim());
        }

        Ok(Self {
            child,
            stdin,
            stdout: stdout_reader,
            model: config.model.clone(),
        })
    }

    pub fn recognize(
        &mut self,
        image_path: &Path,
        project: &str,
    ) -> anyhow::Result<RecognitionResult> {
        if !self.is_alive() {
            anyhow::bail!("worker process has exited");
        }

        let request = serde_json::json!({
            "image": image_path.display().to_string(),
            "project": project,
        });

        writeln!(self.stdin, "{request}")
            .with_context(|| "failed to write request to worker")?;
        self.stdin
            .flush()
            .with_context(|| "failed to flush worker stdin")?;

        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .with_context(|| "failed to read worker response")?;

        if line.trim().is_empty() {
            anyhow::bail!("worker returned empty response");
        }

        let result: RecognitionResult = serde_json::from_str(&line)
            .with_context(|| format!("failed to parse worker response: {line}"))?;

        Ok(result)
    }

    pub fn shutdown(&mut self) -> anyhow::Result<()> {
        let _ = writeln!(self.stdin, "{}", serde_json::json!({"action": "shutdown"}));
        let _ = self.stdin.flush();
        let _ = self.child.wait();
        Ok(())
    }

    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

impl Drop for ResidentWorker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

static RESIDENT_WORKER: OnceLock<Mutex<Option<ResidentWorker>>> = OnceLock::new();

pub fn ensure_resident_worker(config: &AppConfig) {
    if std::env::var("MINDBACK_MLX_RESIDENT").ok().as_deref() == Some("0") {
        return;
    }
    let mut worker = RESIDENT_WORKER.get_or_init(|| Mutex::new(None)).lock().unwrap();
    if worker.is_none() {
        match ResidentWorker::new(config) {
            Ok(w) => {
                eprintln!("MLX resident worker started (model={})", w.model());
                *worker = Some(w);
            }
            Err(e) => {
                eprintln!("Failed to start MLX resident worker: {e}");
            }
        }
    }
}

pub fn restart_resident_worker(config: &AppConfig) {
    if std::env::var("MINDBACK_MLX_RESIDENT").ok().as_deref() == Some("0") {
        return;
    }
    let mut worker = RESIDENT_WORKER.get_or_init(|| Mutex::new(None)).lock().unwrap();
    if worker.is_some() {
        eprintln!("Restarting MLX resident worker...");
    }
    if let Some(mut w) = worker.take() {
        let _ = w.shutdown();
    }
    match ResidentWorker::new(config) {
        Ok(w) => {
            eprintln!("MLX resident worker restarted (model={})", w.model());
            *worker = Some(w);
        }
        Err(e) => {
            eprintln!("Failed to restart MLX resident worker: {e}");
        }
    }
}

// ─────────────────────────────────────────────────────────────
// RecognitionService
// ─────────────────────────────────────────────────────────────

pub struct RecognitionService;

impl RecognitionService {
    pub fn recognize(image_path: &Path, config: &AppConfig) -> RecognitionResult {
        if std::env::var("MINDBACK_MLX_RESIDENT").ok().as_deref() != Some("0") {
            if let Some(result) = Self::try_recognize_resident(image_path, config) {
                return result;
            }
        }
        Self::recognize_with_worker(image_path, config)
    }

    fn try_recognize_resident(
        image_path: &Path,
        config: &AppConfig,
    ) -> Option<RecognitionResult> {
        let mut guard = RESIDENT_WORKER
            .get_or_init(|| Mutex::new(None))
            .try_lock()
            .ok()?;
        let worker = guard.as_mut()?;

        if worker.model() != config.model {
            // Model mismatch — destroy so the next call triggers a rebuild
            // or the caller falls back to one-shot mode.
            *guard = None;
            return None;
        }

        let project = if config.project_name.trim().is_empty() {
            "今日项目"
        } else {
            config.project_name.as_str()
        };

        match worker.recognize(image_path, project) {
            Ok(result) => Some(result),
            Err(error) => {
                eprintln!("Resident worker recognition failed: {error}");
                *guard = None;
                None
            }
        }
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
        std::env::set_var("MINDBACK_MLX_RESIDENT", "0");
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

        std::env::remove_var("MINDBACK_MLX_RESIDENT");
        std::env::remove_var("MINDBACK_MLX_PYTHON");
        std::env::remove_var("MINDBACK_WORKER_PATH");

        assert_eq!(result.intent, "正在查看项目文档");
        assert_eq!(result.visible_context, "屏幕中显示项目文档");
        assert_eq!(result.error, None);
    }
}
