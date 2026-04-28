use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};

use crate::models::{AppConfig, LogEntry};

#[derive(Debug, Clone)]
pub struct Storage {
    base_dir: PathBuf,
}

impl Storage {
    pub fn default_location() -> Result<Self> {
        let base_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("MindBack");
        Self::new(base_dir)
    }

    pub fn new(base_dir: impl Into<PathBuf>) -> Result<Self> {
        let storage = Self {
            base_dir: base_dir.into(),
        };
        fs::create_dir_all(&storage.base_dir)
            .context("failed to create MindBack data directory")?;
        Ok(storage)
    }

    fn config_path(&self) -> PathBuf {
        self.base_dir.join("config.json")
    }

    pub fn read_config(&self) -> Result<AppConfig> {
        let path = self.config_path();
        if !path.exists() {
            return Ok(AppConfig::default());
        }

        let bytes =
            fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let config = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(config)
    }

    pub fn save_config(&self, config: &AppConfig) -> Result<()> {
        fs::create_dir_all(&self.base_dir)?;
        let json = serde_json::to_vec_pretty(config)?;
        fs::write(self.config_path(), json)?;
        Ok(())
    }

    pub fn today_dir(&self) -> Result<PathBuf> {
        self.day_dir(Local::now().date_naive())
    }

    pub fn day_dir(&self, date: NaiveDate) -> Result<PathBuf> {
        let day_dir = self
            .base_dir
            .join("days")
            .join(date.format("%Y-%m-%d").to_string());
        fs::create_dir_all(day_dir.join("thumbs"))?;
        Ok(day_dir)
    }

    pub fn append_log_entry(&self, entry: &LogEntry) -> Result<()> {
        let day_dir = self.day_dir(entry.timestamp.date_naive())?;
        let path = day_dir.join("log.jsonl");
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        let json = serde_json::to_string(entry)?;
        writeln!(file, "{json}")?;
        Ok(())
    }

    pub fn list_today_entries(&self) -> Result<Vec<LogEntry>> {
        self.list_entries_for(Local::now().date_naive())
    }

    pub fn read_today_thumb(&self, relative_path: &str) -> Result<Vec<u8>> {
        let relative_path = Path::new(relative_path);
        if relative_path.as_os_str().is_empty()
            || relative_path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            anyhow::bail!("invalid screenshot thumbnail path");
        }

        let path = self.today_dir()?.join(relative_path);
        fs::read(&path).with_context(|| format!("failed to read {}", path.display()))
    }

    pub fn list_entries_for(&self, date: NaiveDate) -> Result<Vec<LogEntry>> {
        let path = self.day_dir(date)?.join("log.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            entries.push(serde_json::from_str(&line)?);
        }
        Ok(entries)
    }

    pub fn write_today_summary(&self) -> Result<PathBuf> {
        let today = Local::now().date_naive();
        let entries = self.list_entries_for(today)?;
        let day_dir = self.day_dir(today)?;
        let path = day_dir.join("summary.md");
        let on_project = entries.iter().filter(|entry| entry.is_on_project).count();
        let ratio = if entries.is_empty() {
            0
        } else {
            (on_project * 100) / entries.len()
        };

        let mut content = String::new();
        content.push_str("# MindBack 今日监督日志\n\n");
        content.push_str(&format!("- 日期：{}\n", today.format("%Y-%m-%d")));
        content.push_str(&format!("- 总记录数：{}\n", entries.len()));
        content.push_str(&format!("- 符合今日项目：{}%\n\n", ratio));
        content.push_str("## 时间线\n\n");

        for entry in entries {
            content.push_str(&format!(
                "- {} | {} | {}% | {}\n",
                entry.timestamp.format("%H:%M:%S"),
                if entry.is_on_project {
                    "符合"
                } else {
                    "偏离"
                },
                (entry.confidence * 100.0).round() as u8,
                entry.intent
            ));
            content.push_str(&format!("  - 原因：{}\n", entry.reason));
            content.push_str(&format!("  - 缩略图：{}\n", entry.screenshot_thumb));
        }

        fs::write(&path, content)?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use chrono::Local;
    use tempfile::tempdir;

    use super::Storage;
    use crate::models::{AppConfig, LogEntry};

    #[test]
    fn missing_config_returns_defaults() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path()).unwrap();

        assert_eq!(storage.read_config().unwrap(), AppConfig::default());
    }

    #[test]
    fn config_round_trips() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path()).unwrap();
        let config = AppConfig {
            project_name: "MindBack MVP".to_string(),
            project_description: "Build the first loop".to_string(),
            interval_seconds: 30,
            model: "mlx-community/gemma-4-e4b-it-4bit".to_string(),
        };

        storage.save_config(&config).unwrap();

        assert_eq!(storage.read_config().unwrap(), config);
    }

    #[test]
    fn jsonl_entries_append_and_load() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path()).unwrap();
        let entry = LogEntry {
            timestamp: Local::now(),
            project: "MindBack".to_string(),
            screenshot_thumb: "thumbs/example.jpg".to_string(),
            model: "model".to_string(),
            intent: "Testing".to_string(),
            is_on_project: true,
            confidence: 0.9,
            reason: "Test entry".to_string(),
            visible_context: "Unit test".to_string(),
            error: None,
        };

        storage.append_log_entry(&entry).unwrap();
        let entries = storage.list_today_entries().unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].intent, "Testing");
    }
}
