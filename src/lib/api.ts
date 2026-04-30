import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  AppStatus,
  LogEntry,
  SummaryTimeBlock,
  TodaySummaryReport,
} from "./types";

const DEV_BRIDGE_BASE_URL = "http://127.0.0.1:1421/api";

type DevBridgePath =
  | "/config"
  | "/status"
  | "/start-recording"
  | "/stop-recording"
  | "/record-once"
  | "/today-entries"
  | "/today-thumbnail"
  | "/summary"
  | "/summary-report"
  | "/summary-blocks"
  | "/summarize-previous-half-hour";

function isTauriRuntime() {
  return (
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
  );
}

async function requestDevBridge<T>(
  path: DevBridgePath,
  options: RequestInit = {},
  query?: URLSearchParams,
) {
  const url = new URL(`${DEV_BRIDGE_BASE_URL}${path}`);
  if (query) {
    url.search = query.toString();
  }

  const response = await fetch(url, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...options.headers,
    },
  });

  if (!response.ok) {
    throw new Error(await response.text());
  }

  return response.json() as Promise<T>;
}

export function getConfig() {
  if (!isTauriRuntime()) {
    return requestDevBridge<AppConfig>("/config");
  }

  return invoke<AppConfig>("get_config");
}

export function saveConfig(config: AppConfig) {
  if (!isTauriRuntime()) {
    return requestDevBridge<AppConfig>("/config", {
      method: "POST",
      body: JSON.stringify(config),
    });
  }

  return invoke<AppConfig>("save_config", { config });
}

export function getStatus() {
  if (!isTauriRuntime()) {
    return requestDevBridge<AppStatus>("/status");
  }

  return invoke<AppStatus>("get_status");
}

export function startRecording() {
  if (!isTauriRuntime()) {
    return requestDevBridge<AppStatus>("/start-recording", { method: "POST" });
  }

  return invoke<AppStatus>("start_recording");
}

export function stopRecording() {
  if (!isTauriRuntime()) {
    return requestDevBridge<AppStatus>("/stop-recording", { method: "POST" });
  }

  return invoke<AppStatus>("stop_recording");
}

export function recordOnce() {
  if (!isTauriRuntime()) {
    return requestDevBridge<LogEntry>("/record-once", { method: "POST" });
  }

  return invoke<LogEntry>("record_once");
}

export function listTodayEntries() {
  if (!isTauriRuntime()) {
    return requestDevBridge<LogEntry[]>("/today-entries");
  }

  return invoke<LogEntry[]>("list_today_entries");
}

export function getTodayThumbnail(screenshotThumb: string) {
  if (!isTauriRuntime()) {
    return requestDevBridge<string>(
      "/today-thumbnail",
      {},
      new URLSearchParams({ screenshot_thumb: screenshotThumb }),
    );
  }

  return invoke<string>("get_today_thumbnail", { screenshotThumb });
}

export function generateSummary() {
  if (!isTauriRuntime()) {
    return requestDevBridge<string>("/summary", { method: "POST" });
  }

  return invoke<string>("generate_summary");
}

export function generateSummaryReport() {
  if (!isTauriRuntime()) {
    return requestDevBridge<TodaySummaryReport>("/summary-report", {
      method: "POST",
    });
  }

  return invoke<TodaySummaryReport>("generate_summary_report");
}

export function getTodaySummaryBlocks() {
  if (!isTauriRuntime()) {
    return requestDevBridge<SummaryTimeBlock[]>("/summary-blocks");
  }

  return invoke<SummaryTimeBlock[]>("get_today_summary_blocks");
}

export function summarizePreviousHalfHour() {
  if (!isTauriRuntime()) {
    return requestDevBridge<SummaryTimeBlock | null>(
      "/summarize-previous-half-hour",
      { method: "POST" },
    );
  }

  return invoke<SummaryTimeBlock | null>("summarize_previous_half_hour");
}
