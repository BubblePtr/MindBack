import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, AppStatus, LogEntry } from "./types";

export function getConfig() {
  return invoke<AppConfig>("get_config");
}

export function saveConfig(config: AppConfig) {
  return invoke<AppConfig>("save_config", { config });
}

export function getStatus() {
  return invoke<AppStatus>("get_status");
}

export function startRecording() {
  return invoke<AppStatus>("start_recording");
}

export function stopRecording() {
  return invoke<AppStatus>("stop_recording");
}

export function recordOnce() {
  return invoke<LogEntry>("record_once");
}

export function listTodayEntries() {
  return invoke<LogEntry[]>("list_today_entries");
}

export function getTodayThumbnail(screenshotThumb: string) {
  return invoke<string>("get_today_thumbnail", { screenshotThumb });
}

export function generateSummary() {
  return invoke<string>("generate_summary");
}
