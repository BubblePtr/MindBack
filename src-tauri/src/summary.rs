use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    process::{Command, Stdio},
    time::Instant,
};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Local, NaiveDate, Timelike};

use crate::{
    models::{
        AppConfig, LogEntry, NotableDrift, ProjectAlignment, SummaryAgentRequest,
        SummaryAgentResult, SummaryAssessment, SummaryBlockStatus, SummaryLogEntry,
        SummaryTimeBlock, TodaySummaryReport,
    },
    storage::Storage,
};

const SUMMARY_BLOCKS_FILE: &str = "summary_blocks.json";

pub struct SummaryService<'a> {
    storage: &'a Storage,
}

impl<'a> SummaryService<'a> {
    pub fn new(storage: &'a Storage) -> Self {
        Self { storage }
    }

    pub fn today_summary_blocks(&self) -> Result<Vec<SummaryTimeBlock>> {
        read_summary_blocks_for(self.storage, Local::now().date_naive())
    }

    pub fn summarize_previous_half_hour(
        &self,
        config: &AppConfig,
    ) -> Result<Option<SummaryTimeBlock>> {
        summarize_previous_half_hour_at(self.storage, config, Local::now())
    }

    pub fn write_today_summary(&self) -> Result<std::path::PathBuf> {
        Ok(std::path::PathBuf::from(
            self.write_today_summary_report()?.path,
        ))
    }

    pub fn write_today_summary_report(&self) -> Result<TodaySummaryReport> {
        let today = Local::now().date_naive();
        let config = self.storage.read_config()?;
        self.ensure_completed_window_summaries(config)?;
        let entries = self.storage.list_entries_for(today)?;
        let blocks = read_summary_blocks_for(self.storage, today)?;
        let config = self.storage.read_config()?;
        let result = if config.summary_enabled {
            let started_at = Instant::now();
            match run_summary_agent_daily(&config, today, &blocks) {
                Ok(result) => {
                    write_summary_agent_log(
                        self.storage,
                        today,
                        &format!(
                            "daily success elapsed_ms={} blocks={} entries_sent=0 model={}",
                            started_at.elapsed().as_millis(),
                            blocks.len(),
                            config.summary_model
                        ),
                    )?;
                    result
                }
                Err(error) => {
                    let error = error.to_string();
                    write_summary_agent_log(
                        self.storage,
                        today,
                        &format!(
                            "daily fallback elapsed_ms={} blocks={} entries_sent=0 model={} error={}",
                            started_at.elapsed().as_millis(),
                            blocks.len(),
                            config.summary_model,
                            error
                        ),
                    )?;
                    fallback_daily_result(&entries, &blocks, Some(error))
                }
            }
        } else {
            fallback_daily_result(&entries, &blocks, None)
        };
        let path = write_summary_markdown(self.storage, today, &entries, &result)?;
        Ok(TodaySummaryReport {
            path: path.display().to_string(),
            result,
        })
    }

    #[cfg(test)]
    pub fn fallback_block(
        start: DateTime<Local>,
        end: DateTime<Local>,
        entries: &[LogEntry],
    ) -> SummaryTimeBlock {
        fallback_block(start, end, entries, None)
    }

    fn ensure_completed_window_summaries(&self, config: AppConfig) -> Result<()> {
        let now = Local::now();
        let today = now.date_naive();
        let entries = self.storage.list_entries_for(today)?;
        if entries.is_empty() {
            return Ok(());
        }

        let mut by_start = BTreeMap::<DateTime<Local>, Vec<LogEntry>>::new();
        let current_start = bucket_start(now);
        for entry in entries {
            let start = bucket_start(entry.timestamp);
            if start < current_start {
                by_start.entry(start).or_default().push(entry);
            }
        }

        let cached = read_summary_blocks_for(self.storage, today)?;
        for (start, entries) in by_start {
            let end = start + Duration::minutes(30);
            if has_cached_block(&cached, start, end) {
                continue;
            }
            let block = summarize_window(&config, start, end, &entries);
            write_cached_summary_block_for(self.storage, today, &block)?;
        }

        Ok(())
    }
}

pub fn bucket_start(timestamp: DateTime<Local>) -> DateTime<Local> {
    let minute = if timestamp.minute() < 30 { 0 } else { 30 };
    timestamp
        .with_minute(minute)
        .and_then(|time| time.with_second(0))
        .and_then(|time| time.with_nanosecond(0))
        .expect("valid half-hour bucket")
}

pub fn previous_completed_window(now: DateTime<Local>) -> (DateTime<Local>, DateTime<Local>) {
    let end = bucket_start(now);
    let start = end - Duration::minutes(30);
    (start, end)
}

pub fn summarize_previous_half_hour_at(
    storage: &Storage,
    config: &AppConfig,
    now: DateTime<Local>,
) -> Result<Option<SummaryTimeBlock>> {
    let (start, end) = previous_completed_window(now);
    let date = start.date_naive();
    let cached = read_summary_blocks_for(storage, date)?;
    if let Some(block) = cached
        .into_iter()
        .find(|block| block.start == format_time(start) && block.end == format_time(end))
    {
        return Ok(Some(block));
    }

    let entries: Vec<_> = storage
        .list_entries_for(date)?
        .into_iter()
        .filter(|entry| entry.timestamp >= start && entry.timestamp < end)
        .collect();

    let block = if entries.is_empty() {
        fallback_block(start, end, &entries, None)
    } else {
        summarize_window(config, start, end, &entries)
    };
    write_cached_summary_block_for(storage, date, &block)?;

    Ok(Some(block))
}

pub fn write_cached_summary_block_for(
    storage: &Storage,
    date: NaiveDate,
    block: &SummaryTimeBlock,
) -> Result<()> {
    let mut blocks = read_summary_blocks_for(storage, date)?;
    blocks.retain(|existing| existing.start != block.start || existing.end != block.end);
    blocks.push(block.clone());
    blocks.sort_by(|left, right| left.start.cmp(&right.start).then(left.end.cmp(&right.end)));

    let path = summary_blocks_path(storage, date)?;
    fs::write(path, serde_json::to_vec_pretty(&blocks)?)?;
    Ok(())
}

fn read_summary_blocks_for(storage: &Storage, date: NaiveDate) -> Result<Vec<SummaryTimeBlock>> {
    let path = summary_blocks_path(storage, date)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut blocks: Vec<SummaryTimeBlock> = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    blocks.sort_by(|left, right| left.start.cmp(&right.start).then(left.end.cmp(&right.end)));
    Ok(blocks)
}

fn summary_blocks_path(storage: &Storage, date: NaiveDate) -> Result<std::path::PathBuf> {
    Ok(storage.day_dir(date)?.join(SUMMARY_BLOCKS_FILE))
}

fn has_cached_block(
    cached: &[SummaryTimeBlock],
    start: DateTime<Local>,
    end: DateTime<Local>,
) -> bool {
    let start = format_time(start);
    let end = format_time(end);
    cached
        .iter()
        .any(|block| block.start == start && block.end == end)
}

fn summarize_window(
    config: &AppConfig,
    start: DateTime<Local>,
    end: DateTime<Local>,
    entries: &[LogEntry],
) -> SummaryTimeBlock {
    if !config.summary_enabled {
        return fallback_block(start, end, entries, None);
    }

    match run_summary_agent_window(config, start, end, entries) {
        Ok(Some(mut block)) => {
            block.start = format_time(start);
            block.end = format_time(end);
            block.record_count = entries.len();
            block.on_project_ratio = on_project_ratio(entries);
            block
        }
        Ok(None) => fallback_block(start, end, entries, None),
        Err(error) => fallback_block(start, end, entries, Some(error.to_string())),
    }
}

fn run_summary_agent_window(
    config: &AppConfig,
    start: DateTime<Local>,
    end: DateTime<Local>,
    entries: &[LogEntry],
) -> Result<Option<SummaryTimeBlock>> {
    let request = SummaryAgentRequest {
        task: "window".to_string(),
        date: start.date_naive().format("%Y-%m-%d").to_string(),
        project: config.project_name.clone(),
        window_start: Some(format_time(start)),
        window_end: Some(format_time(end)),
        entries: entries.iter().map(summary_log_entry).collect(),
        time_blocks: Vec::new(),
    };
    let result = run_summary_agent(config, &request)?;
    if let Some(error) = result.error {
        anyhow::bail!(error);
    }
    Ok(result.time_blocks.into_iter().next())
}

fn run_summary_agent_daily(
    config: &AppConfig,
    today: NaiveDate,
    blocks: &[SummaryTimeBlock],
) -> Result<SummaryAgentResult> {
    let request = daily_summary_agent_request(config, today, blocks);
    let mut result = run_summary_agent(config, &request)?;
    if let Some(error) = &result.error {
        anyhow::bail!(error.clone());
    }
    if result.time_blocks.is_empty() {
        result.time_blocks = blocks.to_vec();
    }
    Ok(result)
}

fn daily_summary_agent_request(
    config: &AppConfig,
    today: NaiveDate,
    blocks: &[SummaryTimeBlock],
) -> SummaryAgentRequest {
    SummaryAgentRequest {
        task: "daily".to_string(),
        date: today.format("%Y-%m-%d").to_string(),
        project: config.project_name.clone(),
        window_start: None,
        window_end: None,
        entries: Vec::new(),
        time_blocks: blocks.iter().map(compact_daily_block).collect(),
    }
}

fn compact_daily_block(block: &SummaryTimeBlock) -> SummaryTimeBlock {
    SummaryTimeBlock {
        start: block.start.clone(),
        end: block.end.clone(),
        status: block.status.clone(),
        summary: block.summary.clone(),
        evidence: block.evidence.iter().take(1).cloned().collect(),
        record_count: block.record_count,
        on_project_ratio: block.on_project_ratio,
        error: block.error.clone(),
    }
}

fn run_summary_agent(
    config: &AppConfig,
    request: &SummaryAgentRequest,
) -> Result<SummaryAgentResult> {
    let command =
        std::env::var("MINDBACK_SUMMARY_AGENT_COMMAND").unwrap_or_else(|_| "bun".to_string());
    let agent_path = std::env::var("MINDBACK_SUMMARY_AGENT_PATH")
        .unwrap_or_else(|_| "workers/summary_agent.ts".to_string());
    run_summary_agent_command(config, request, &command, &agent_path)
}

fn run_summary_agent_command(
    config: &AppConfig,
    request: &SummaryAgentRequest,
    command: &str,
    agent_path: &str,
) -> Result<SummaryAgentResult> {
    let default_cwd = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let cwd = std::env::var("MINDBACK_SUMMARY_AGENT_CWD")
        .map(std::path::PathBuf::from)
        .unwrap_or(default_cwd);

    let mut child = Command::new(command)
        .arg(agent_path)
        .current_dir(cwd)
        .env("MINDBACK_SUMMARY_MODEL", &config.summary_model)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to launch Summary Agent")?;

    let stdin = child
        .stdin
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("failed to open Summary Agent stdin"))?;
    stdin.write_all(serde_json::to_vec(&request)?.as_slice())?;

    let output = child.wait_with_output()?;
    if !output.status.success() {
        anyhow::bail!(
            "Summary Agent exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "failed to parse Summary Agent output: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn fallback_block(
    start: DateTime<Local>,
    end: DateTime<Local>,
    entries: &[LogEntry],
    error: Option<String>,
) -> SummaryTimeBlock {
    if entries.is_empty() {
        return SummaryTimeBlock {
            start: format_time(start),
            end: format_time(end),
            status: SummaryBlockStatus::InsufficientData,
            summary: "该时间段没有记录。".to_string(),
            evidence: Vec::new(),
            record_count: 0,
            on_project_ratio: 0,
            error,
        };
    }

    let ratio = on_project_ratio(entries);
    let status = if ratio >= 60 {
        SummaryBlockStatus::OnProject
    } else if ratio == 0 {
        SummaryBlockStatus::OffProject
    } else {
        SummaryBlockStatus::Uncertain
    };
    let intent = dominant_intent(entries);
    let mut evidence: Vec<String> = entries
        .iter()
        .filter_map(|entry| {
            if !entry.visible_context.trim().is_empty() {
                return Some(entry.visible_context.clone());
            }
            if !entry.reason.trim().is_empty() {
                return Some(entry.reason.clone());
            }
            None
        })
        .take(2)
        .collect();
    if let Some(error) = &error {
        evidence.push(format!("Summary Agent 不可用：{error}"));
    }

    SummaryTimeBlock {
        start: format_time(start),
        end: format_time(end),
        status,
        summary: intent,
        evidence,
        record_count: entries.len(),
        on_project_ratio: ratio,
        error,
    }
}

fn fallback_daily_result(
    entries: &[LogEntry],
    blocks: &[SummaryTimeBlock],
    error: Option<String>,
) -> SummaryAgentResult {
    let ratio = on_project_ratio(entries);
    let assessment = if entries.is_empty() {
        SummaryAssessment::InsufficientData
    } else if ratio >= 70 {
        SummaryAssessment::Focused
    } else if ratio >= 40 {
        SummaryAssessment::Mixed
    } else {
        SummaryAssessment::Drifted
    };
    let overview = if entries.is_empty() {
        "今天还没有可用于总结的记录。".to_string()
    } else {
        format!(
            "今天共有 {} 条记录，{}% 符合今日项目。",
            entries.len(),
            ratio
        )
    };
    let notable_drifts = blocks
        .iter()
        .filter(|block| matches!(block.status, SummaryBlockStatus::OffProject))
        .map(|block| NotableDrift {
            time: format!("{} - {}", block.start, block.end),
            reason: block.summary.clone(),
        })
        .collect();

    SummaryAgentResult {
        overview,
        project_alignment: ProjectAlignment {
            on_project_ratio: ratio,
            assessment,
        },
        time_blocks: blocks.to_vec(),
        notable_drifts,
        reflection_prompts: vec![
            "哪些时间段最容易偏离今日项目？".to_string(),
            "明天是否需要提前安排一个低干扰的启动任务？".to_string(),
        ],
        error,
    }
}

fn write_summary_agent_log(storage: &Storage, today: NaiveDate, message: &str) -> Result<()> {
    let path = storage.day_dir(today)?.join("summary_agent.log");
    let timestamp = Local::now().to_rfc3339();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "[{timestamp}] {message}")?;
    Ok(())
}

fn write_summary_markdown(
    storage: &Storage,
    today: NaiveDate,
    entries: &[LogEntry],
    result: &SummaryAgentResult,
) -> Result<std::path::PathBuf> {
    let day_dir = storage.day_dir(today)?;
    let path = day_dir.join("summary.md");
    let project = entries
        .iter()
        .find_map(|entry| {
            if entry.project.trim().is_empty() {
                None
            } else {
                Some(entry.project.as_str())
            }
        })
        .unwrap_or("未设置");

    let mut content = String::new();
    content.push_str("# MindBack 今日监督日志\n\n");
    content.push_str(&format!("- 日期：{}\n", today.format("%Y-%m-%d")));
    content.push_str(&format!("- 今日项目：{}\n", project));
    content.push_str(&format!("- 总记录数：{}\n", entries.len()));
    content.push_str(&format!(
        "- 符合今日项目：{}%\n\n",
        result.project_alignment.on_project_ratio
    ));
    content.push_str("## 总览\n\n");
    content.push_str(&result.overview);
    content.push_str("\n\n## 时间段摘要\n\n");

    if result.time_blocks.is_empty() {
        content.push_str("- 暂无已完成半小时窗口摘要。\n");
    } else {
        for block in &result.time_blocks {
            content.push_str(&format!(
                "- {} - {} | {} | {}% | {}\n",
                block.start,
                block.end,
                status_label(&block.status),
                block.on_project_ratio,
                block.summary
            ));
            for evidence in block.evidence.iter().take(2) {
                content.push_str(&format!("  - 依据：{}\n", evidence));
            }
        }
    }

    if !result.notable_drifts.is_empty() {
        content.push_str("\n## 明显偏离或不确定片段\n\n");
        for drift in &result.notable_drifts {
            content.push_str(&format!("- {}：{}\n", drift.time, drift.reason));
        }
    }

    content.push_str("\n## 复盘问题\n\n");
    for prompt in &result.reflection_prompts {
        content.push_str(&format!("- {}\n", prompt));
    }
    content.push_str("\n> 由 AI 总结生成，基于本地监督日志；请以原始记录为准。\n");

    fs::write(&path, content)?;
    Ok(path)
}

fn summary_log_entry(entry: &LogEntry) -> SummaryLogEntry {
    SummaryLogEntry {
        timestamp: entry.timestamp.to_rfc3339(),
        intent: entry.intent.clone(),
        is_on_project: entry.is_on_project,
        confidence: entry.confidence,
        reason: entry.reason.clone(),
        visible_context: entry.visible_context.clone(),
        error: entry.error.clone(),
    }
}

fn on_project_ratio(entries: &[LogEntry]) -> u8 {
    if entries.is_empty() {
        return 0;
    }
    let on_project = entries.iter().filter(|entry| entry.is_on_project).count();
    ((on_project * 100) / entries.len()) as u8
}

fn dominant_intent(entries: &[LogEntry]) -> String {
    let mut counts = BTreeMap::<&str, usize>::new();
    for entry in entries {
        *counts.entry(entry.intent.as_str()).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(intent, _)| intent.to_string())
        .unwrap_or_else(|| "暂无判断".to_string())
}

fn format_time(time: DateTime<Local>) -> String {
    time.format("%H:%M").to_string()
}

fn status_label(status: &SummaryBlockStatus) -> &'static str {
    match status {
        SummaryBlockStatus::OnProject => "符合",
        SummaryBlockStatus::OffProject => "偏离",
        SummaryBlockStatus::Uncertain => "不确定",
        SummaryBlockStatus::InsufficientData => "无数据",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::{Datelike, Local, TimeZone};
    use tempfile::tempdir;

    use crate::{
        models::{
            AppConfig, LogEntry, SummaryAgentRequest, SummaryAssessment, SummaryBlockStatus,
        },
        storage::Storage,
        summary::{
            bucket_start, previous_completed_window, summarize_previous_half_hour_at,
            write_cached_summary_block_for, SummaryService,
        },
    };

    fn at(hour: u32, minute: u32) -> chrono::DateTime<Local> {
        let today = Local::now().date_naive();
        Local
            .with_ymd_and_hms(today.year(), today.month(), today.day(), hour, minute, 0)
            .single()
            .unwrap()
    }

    fn entry_at(hour: u32, minute: u32, intent: &str, on_project: bool) -> LogEntry {
        LogEntry {
            timestamp: at(hour, minute),
            project: "MindBack".to_string(),
            screenshot_thumb: format!("thumbs/{hour:02}-{minute:02}.jpg"),
            model: "model".to_string(),
            intent: intent.to_string(),
            is_on_project: on_project,
            confidence: 0.8,
            reason: "visible work summary".to_string(),
            visible_context: "editor".to_string(),
            error: None,
        }
    }

    #[test]
    fn previous_completed_window_uses_last_closed_half_hour() {
        let now = at(10, 17);
        let (start, end) = previous_completed_window(now);

        assert_eq!(start, at(9, 30));
        assert_eq!(end, at(10, 0));
    }

    #[test]
    fn bucket_start_rounds_down_to_half_hour() {
        assert_eq!(bucket_start(at(9, 44)), at(9, 30));
        assert_eq!(bucket_start(at(10, 0)), at(10, 0));
    }

    #[test]
    fn previous_half_hour_summary_ignores_current_window_entries() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path()).unwrap();
        let config = AppConfig {
            project_name: "MindBack".to_string(),
            summary_enabled: false,
            ..AppConfig::default()
        };
        storage
            .append_log_entry(&entry_at(9, 40, "Writing summary design", true))
            .unwrap();
        storage
            .append_log_entry(&entry_at(10, 5, "Current window work", false))
            .unwrap();

        let block = summarize_previous_half_hour_at(&storage, &config, at(10, 17))
            .unwrap()
            .unwrap();

        assert_eq!(block.start, "09:30");
        assert_eq!(block.end, "10:00");
        assert_eq!(block.record_count, 1);
        assert_eq!(block.status, SummaryBlockStatus::OnProject);
        assert!(block.summary.contains("Writing summary design"));
    }

    #[test]
    fn previous_half_hour_without_entries_caches_insufficient_data_without_sidecar() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path()).unwrap();

        let block = summarize_previous_half_hour_at(&storage, &AppConfig::default(), at(10, 17))
            .unwrap()
            .unwrap();

        assert_eq!(block.start, "09:30");
        assert_eq!(block.status, SummaryBlockStatus::InsufficientData);
        assert_eq!(block.record_count, 0);

        let cached = SummaryService::new(&storage)
            .today_summary_blocks()
            .unwrap();
        assert_eq!(cached, vec![block]);
    }

    #[test]
    fn cached_block_is_returned_without_regenerating() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path()).unwrap();
        let cached = SummaryService::fallback_block(
            at(9, 30),
            at(10, 0),
            &[entry_at(9, 40, "Cached work", true)],
        );
        write_cached_summary_block_for(&storage, at(9, 30).date_naive(), &cached).unwrap();

        let block = summarize_previous_half_hour_at(&storage, &AppConfig::default(), at(10, 17))
            .unwrap()
            .unwrap();

        assert_eq!(block.summary, cached.summary);
        assert_eq!(block.record_count, 1);
    }

    #[test]
    fn today_summary_report_returns_structured_result_and_writes_markdown() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path()).unwrap();
        storage
            .save_config(&AppConfig {
                project_name: "MindBack".to_string(),
                summary_enabled: false,
                ..AppConfig::default()
            })
            .unwrap();
        storage
            .append_log_entry(&entry_at(9, 40, "Writing UI integration", true))
            .unwrap();

        let report = SummaryService::new(&storage)
            .write_today_summary_report()
            .unwrap();

        assert!(report.path.ends_with("summary.md"));
        assert!(std::path::Path::new(&report.path).exists());
        assert_eq!(
            report.result.project_alignment.assessment,
            SummaryAssessment::Focused
        );
        assert!(report.result.overview.contains("1 条记录"));
    }

    #[test]
    fn daily_summary_agent_request_uses_cached_blocks_without_raw_entries() {
        let block = SummaryService::fallback_block(
            at(9, 30),
            at(10, 0),
            &[entry_at(9, 40, "Writing UI integration", true)],
        );

        let request = super::daily_summary_agent_request(
            &AppConfig {
                project_name: "MindBack".to_string(),
                ..AppConfig::default()
            },
            at(9, 30).date_naive(),
            &[block],
        );

        assert_eq!(request.task, "daily");
        assert!(request.entries.is_empty());
        assert_eq!(request.time_blocks.len(), 1);
        assert_eq!(request.time_blocks[0].evidence.len(), 1);
    }

    #[test]
    fn summary_agent_command_does_not_require_key_in_parent_process() {
        let dir = tempdir().unwrap();
        let script = dir.path().join("agent.sh");
        fs::write(
            &script,
            r#"cat >/dev/null
printf '%s' '{"overview":"LLM overview","projectAlignment":{"onProjectRatio":100,"assessment":"focused"},"timeBlocks":[],"notableDrifts":[],"reflectionPrompts":["Review prompt"],"error":null}'
"#,
        )
        .unwrap();
        let request = SummaryAgentRequest {
            task: "daily".to_string(),
            date: Local::now().date_naive().format("%Y-%m-%d").to_string(),
            project: "MindBack".to_string(),
            window_start: None,
            window_end: None,
            entries: Vec::new(),
            time_blocks: Vec::new(),
        };

        let result = super::run_summary_agent_command(
            &AppConfig::default(),
            &request,
            "/bin/sh",
            script.to_str().unwrap(),
        )
        .unwrap();

        assert_eq!(result.overview, "LLM overview");
    }
}
