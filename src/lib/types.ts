export type AppConfig = {
  project_name: string;
  project_description: string;
  interval_seconds: number;
  model: string;
  summary_model: string;
  summary_provider: string;
  summary_enabled: boolean;
};

export type RecognitionResult = {
  intent: string;
  is_on_project: boolean;
  confidence: number;
  reason: string;
  visible_context: string;
  error: string | null;
};

export type LogEntry = RecognitionResult & {
  timestamp: string;
  project: string;
  screenshot_thumb: string;
  model: string;
  error: string | null;
};

export type AppStatus = {
  is_recording: boolean;
  today: string;
  project_name: string;
  last_error: string | null;
};

export type SummaryBlockStatus =
  | "on_project"
  | "off_project"
  | "uncertain"
  | "insufficient_data";

export type SummaryTimeBlock = {
  start: string;
  end: string;
  status: SummaryBlockStatus;
  summary: string;
  evidence: string[];
  record_count: number;
  on_project_ratio: number;
  error: string | null;
};
