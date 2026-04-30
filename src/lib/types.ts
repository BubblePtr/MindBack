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
  recordCount: number;
  onProjectRatio: number;
  error: string | null;
};

export type SummaryAssessment =
  | "focused"
  | "mixed"
  | "drifted"
  | "insufficient_data";

export type ProjectAlignment = {
  onProjectRatio: number;
  assessment: SummaryAssessment;
};

export type NotableDrift = {
  time: string;
  reason: string;
};

export type SummaryAgentResult = {
  overview: string;
  projectAlignment: ProjectAlignment;
  timeBlocks: SummaryTimeBlock[];
  notableDrifts: NotableDrift[];
  reflectionPrompts: string[];
  error: string | null;
};

export type TodaySummaryReport = {
  path: string;
  result: SummaryAgentResult;
};
