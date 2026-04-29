import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { House, SlidersHorizontal } from "@phosphor-icons/react";
import {
  generateSummary,
  getConfig,
  getStatus,
  getTodaySummaryBlocks,
  getTodayThumbnail,
  listTodayEntries,
  recordOnce,
  saveConfig,
  startRecording,
  stopRecording,
  summarizePreviousHalfHour,
} from "./lib/api";
import type { AppConfig, AppStatus, LogEntry, SummaryTimeBlock } from "./lib/types";
import appIcon from "./assets/mindback-app-icon.png";
import { Button } from "./components/ui/button";
import { Dialog } from "./components/ui/dialog";
import { Field, Input, Textarea } from "./components/ui/field";
import { Select } from "./components/ui/select";
import { Tabs } from "./components/ui/tabs";

const MODELS = [
  "mlx-community/Qwen3-VL-4B-Instruct-4bit",
  "mlx-community/Qwen3-VL-8B-Instruct-4bit",
  "mlx-community/gemma-4-e4b-it-4bit",
];

const INTERVAL_OPTIONS = [30, 60, 120, 300].map((seconds) => ({
  label: `${seconds} 秒`,
  value: String(seconds),
}));

const MODEL_OPTIONS = MODELS.map((model) => ({
  label: model,
  value: model,
}));

const DEFAULT_CONFIG: AppConfig = {
  project_name: "",
  project_description: "",
  interval_seconds: 60,
  model: MODELS[0],
  summary_model: "deepseek-chat",
  summary_provider: "deepseek",
  summary_enabled: true,
};

type ActiveTab = "home" | "settings";
type SummaryPanelTab = "blocks" | "daily";
type TimelineBucket = {
  key: string;
  range: string;
  entries: LogEntry[];
  title: string;
  detail: string;
};

type DailyReportView = {
  alignmentLabel: string;
  overview: string;
  primaryWork: string[];
  driftNotes: string[];
  prompts: string[];
};

function formatEntryTime(timestamp: string) {
  return new Date(timestamp).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatTime(date: Date) {
  return date.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function getBucketStart(timestamp: string) {
  const time = new Date(timestamp);
  const bucketStart = new Date(time);
  bucketStart.setMinutes(Math.floor(time.getMinutes() / 30) * 30, 0, 0);
  return bucketStart;
}

function getBucketEnd(bucketKey: string) {
  const end = new Date(bucketKey);
  end.setMinutes(end.getMinutes() + 30);
  return end;
}

function addHalfHour(date: Date) {
  const next = new Date(date);
  next.setMinutes(next.getMinutes() + 30);
  return next;
}

function isCompletedBucket(bucketKey: string, now: Date) {
  return getBucketEnd(bucketKey).getTime() <= now.getTime();
}

function formatBucketRange(bucketKey: string) {
  const start = new Date(bucketKey);
  const end = getBucketEnd(bucketKey);
  return `${formatTime(start)} - ${formatTime(end)}`;
}

function dominantIntent(entries: LogEntry[]) {
  const counts = new Map<string, number>();
  for (const entry of entries) {
    counts.set(entry.intent, (counts.get(entry.intent) ?? 0) + 1);
  }

  return [...counts.entries()].sort((left, right) => right[1] - left[1])[0]?.[0] ?? "暂无判断";
}

function summarizeBucket(key: string, bucketEntries: LogEntry[]): TimelineBucket {
  if (bucketEntries.length === 0) {
    return {
      key,
      range: formatBucketRange(key),
      entries: [],
      title: "暂无记录",
      detail: "0 条记录。该时间段没有可展示的截图。",
    };
  }

  const onProject = bucketEntries.filter((entry) => entry.is_on_project).length;
  const ratio = Math.round((onProject / bucketEntries.length) * 100);
  const intent = dominantIntent(bucketEntries);
  const context = bucketEntries.find((entry) => entry.visible_context)?.visible_context;

  return {
    key,
    range: formatBucketRange(key),
    entries: bucketEntries,
    title: intent,
    detail: `${bucketEntries.length} 条记录，${ratio}% 符合今日项目${context ? `。${context}` : "。"}`,
  };
}

function summaryStatusLabel(status: SummaryTimeBlock["status"]) {
  switch (status) {
    case "on_project":
      return "符合";
    case "off_project":
      return "偏离";
    case "uncertain":
      return "不确定";
    case "insufficient_data":
      return "无数据";
  }
}

function mergeSummaryBlocks(
  blocks: SummaryTimeBlock[],
  refreshedBlock: SummaryTimeBlock | null,
) {
  if (!refreshedBlock) return blocks;

  const byRange = new Map(blocks.map((block) => [`${block.start}-${block.end}`, block]));
  byRange.set(`${refreshedBlock.start}-${refreshedBlock.end}`, refreshedBlock);
  return [...byRange.values()].sort((left, right) =>
    `${left.start}-${left.end}`.localeCompare(`${right.start}-${right.end}`),
  );
}

function formatError(error: unknown) {
  const text = String(error);
  if (text.includes("Failed to fetch")) {
    return "当前浏览器预览未连接 MindBack 调试后端；请先运行 bun run tauri dev。";
  }
  if (text.includes("invoke")) {
    return "当前浏览器预览未连接 Tauri 后端；请在桌面应用中使用记录功能。";
  }
  return text;
}

function buildDailyReportView(
  entries: LogEntry[],
  blocks: SummaryTimeBlock[],
  projectName: string,
  onProjectRatio: string,
): DailyReportView {
  if (entries.length === 0) {
    return {
      alignmentLabel: "等待记录",
      overview: "今天还没有可复盘的记录。开始记录后，日报会基于时间段摘要生成。",
      primaryWork: [],
      driftNotes: [],
      prompts: ["今天最重要的一件事是什么？"],
    };
  }

  const sortedBlocks = blocks
    .filter((block) => block.record_count > 0)
    .sort((left, right) => right.record_count - left.record_count);
  const primaryWork = sortedBlocks
    .filter((block) => block.status === "on_project")
    .slice(0, 3)
    .map((block) => `${block.start} - ${block.end}：${block.summary}`);
  const driftNotes = blocks
    .filter((block) => block.status === "off_project" || block.status === "uncertain")
    .slice(0, 3)
    .map((block) => `${block.start} - ${block.end}：${block.summary}`);

  return {
    alignmentLabel: Number.parseInt(onProjectRatio, 10) >= 80 ? "整体专注" : "需要复盘",
    overview: `${entries.length} 条记录，${onProjectRatio} 符合今日项目${projectName ? `「${projectName}」` : ""}。`,
    primaryWork:
      primaryWork.length > 0
        ? primaryWork
        : ["已有记录，但还没有生成稳定的时间段摘要。"],
    driftNotes:
      driftNotes.length > 0
        ? driftNotes
        : ["暂未发现明显偏离或不确定片段。"],
    prompts: [
      "今天最值得保留的工作节奏是什么？",
      "下一次开始前，哪个上下文可以提前准备好？",
    ],
  };
}

function App() {
  const [activeTab, setActiveTab] = useState<ActiveTab>("home");
  const [summaryPanelTab, setSummaryPanelTab] = useState<SummaryPanelTab>("blocks");
  const [config, setConfig] = useState<AppConfig>(DEFAULT_CONFIG);
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [summaryBlocks, setSummaryBlocks] = useState<SummaryTimeBlock[]>([]);
  const [currentTime, setCurrentTime] = useState(() => new Date());
  const [selectedBucketKey, setSelectedBucketKey] = useState<string | null>(null);
  const [detailEntry, setDetailEntry] = useState<LogEntry | null>(null);
  const [thumbUrls, setThumbUrls] = useState<Record<string, string>>({});
  const [message, setMessage] = useState<string>("");
  const [isBusy, setBusy] = useState(false);
  const activeTabRef = useRef(activeTab);
  const refreshInFlightRef = useRef(false);
  const lastSummaryBucketRef = useRef<string | null>(null);

  const timelineEntries = useMemo(
    () =>
      entries
        .slice()
        .sort(
          (left, right) =>
            new Date(left.timestamp).getTime() - new Date(right.timestamp).getTime(),
        ),
    [entries],
  );

  const timelineBuckets = useMemo(() => {
    const buckets = new Map<string, LogEntry[]>();
    for (const entry of timelineEntries) {
      const key = getBucketStart(entry.timestamp).toISOString();
      buckets.set(key, [...(buckets.get(key) ?? []), entry]);
    }

    const sortedKeys = [...buckets.keys()].sort();
    if (sortedKeys.length === 0) return [];

    const firstStart = new Date(sortedKeys[0]);
    const lastEntryStart = new Date(sortedKeys[sortedKeys.length - 1]);
    const currentStart = getBucketStart(currentTime.toISOString());
    const lastStart =
      status?.is_recording && currentStart > lastEntryStart ? currentStart : lastEntryStart;
    const completeBuckets: TimelineBucket[] = [];

    for (
      let cursor = firstStart;
      cursor.getTime() <= lastStart.getTime();
      cursor = addHalfHour(cursor)
    ) {
      const key = cursor.toISOString();
      completeBuckets.push(summarizeBucket(key, buckets.get(key) ?? []));
    }

    return completeBuckets;
  }, [currentTime, status?.is_recording, timelineEntries]);

  const selectedBucketIndex = useMemo(() => {
    if (timelineBuckets.length === 0) return 0;
    const index = timelineBuckets.findIndex((bucket) => bucket.key === selectedBucketKey);
    return index >= 0 ? index : timelineBuckets.length - 1;
  }, [selectedBucketKey, timelineBuckets]);

  const selectedBucket = timelineBuckets[selectedBucketIndex] ?? null;
  const visibleEntries = selectedBucket?.entries ?? [];

  const summaryBlockByRange = useMemo(() => {
    const blocks = new Map<string, SummaryTimeBlock>();
    for (const block of summaryBlocks) {
      blocks.set(`${block.start} - ${block.end}`, block);
    }
    return blocks;
  }, [summaryBlocks]);

  const summarySections = useMemo(
    () =>
      timelineBuckets
        .filter((bucket) => isCompletedBucket(bucket.key, currentTime))
        .map((bucket) => {
          const block = summaryBlockByRange.get(bucket.range);
          if (!block) return bucket;
          const evidence = block.evidence.slice(0, 2).join(" ");
          const label = summaryStatusLabel(block.status);
          return {
            ...bucket,
            title: block.summary,
            detail: `${label}，${block.record_count} 条记录，${block.on_project_ratio}% 符合今日项目${evidence ? `。${evidence}` : "。"}`,
          };
        }),
    [currentTime, summaryBlockByRange, timelineBuckets],
  );

  const onProjectRatio = useMemo(() => {
    if (entries.length === 0) return "0%";
    const onProject = entries.filter((entry) => entry.is_on_project).length;
    return `${Math.round((onProject / entries.length) * 100)}%`;
  }, [entries]);

  const dailyReportView = useMemo(
    () =>
      buildDailyReportView(
        entries,
        summaryBlocks,
        config.project_name,
        onProjectRatio,
      ),
    [config.project_name, entries, onProjectRatio, summaryBlocks],
  );

  const detailThumbUrl = detailEntry ? thumbUrls[detailEntry.screenshot_thumb] : "";

  useEffect(() => {
    activeTabRef.current = activeTab;
  }, [activeTab]);

  const refresh = useCallback(async (options: { syncConfig?: boolean } = {}) => {
    const syncConfig = options.syncConfig ?? activeTabRef.current !== "settings";
    const now = new Date();
    const currentBucketKey = getBucketStart(now.toISOString()).toISOString();
    const shouldSummarizePreviousWindow = lastSummaryBucketRef.current !== currentBucketKey;
    lastSummaryBucketRef.current = currentBucketKey;
    const [nextConfig, nextStatus, nextEntries, nextBlocks, refreshedBlock] = await Promise.all([
      getConfig(),
      getStatus(),
      listTodayEntries(),
      getTodaySummaryBlocks(),
      shouldSummarizePreviousWindow
        ? summarizePreviousHalfHour().catch(() => null)
        : Promise.resolve(null),
    ]);
    if (syncConfig) {
      setConfig(nextConfig);
    }
    setCurrentTime(now);
    setStatus(nextStatus);
    setEntries(nextEntries);
    setSummaryBlocks(mergeSummaryBlocks(nextBlocks, refreshedBlock));
  }, []);

  const refreshInBackground = useCallback(
    async (options: { syncConfig?: boolean } = {}) => {
      if (refreshInFlightRef.current) return;
      refreshInFlightRef.current = true;
      try {
        await refresh(options);
      } catch (error) {
        setMessage(formatError(error));
      } finally {
        refreshInFlightRef.current = false;
      }
    },
    [refresh],
  );

  useEffect(() => {
    void refreshInBackground({ syncConfig: true });
  }, [refreshInBackground]);

  useEffect(() => {
    const refreshVisiblePage = () => {
      if (document.visibilityState === "visible") {
        void refreshInBackground();
      }
    };

    const intervalId = window.setInterval(refreshVisiblePage, 2000);
    window.addEventListener("focus", refreshVisiblePage);
    document.addEventListener("visibilitychange", refreshVisiblePage);

    return () => {
      window.clearInterval(intervalId);
      window.removeEventListener("focus", refreshVisiblePage);
      document.removeEventListener("visibilitychange", refreshVisiblePage);
    };
  }, [refreshInBackground]);

  useEffect(() => {
    if (timelineBuckets.length === 0) {
      setSelectedBucketKey(null);
      return;
    }

    if (
      selectedBucketKey === null ||
      !timelineBuckets.some((bucket) => bucket.key === selectedBucketKey)
    ) {
      setSelectedBucketKey(timelineBuckets[timelineBuckets.length - 1].key);
    }
  }, [selectedBucketKey, timelineBuckets]);

  useEffect(() => {
    let isCancelled = false;

    async function loadThumbnails() {
      const missingEntries = timelineEntries.filter(
        (entry) => entry.screenshot_thumb && thumbUrls[entry.screenshot_thumb] === undefined,
      );

      if (missingEntries.length === 0) return;

      const loadedEntries = await Promise.all(
        missingEntries.map(async (entry) => {
          try {
            const url = await getTodayThumbnail(entry.screenshot_thumb);
            return [entry.screenshot_thumb, url] as const;
          } catch {
            return [entry.screenshot_thumb, ""] as const;
          }
        }),
      );

      if (isCancelled) return;

      setThumbUrls((current) => {
        const next = { ...current };
        for (const [path, url] of loadedEntries) {
          next[path] = url;
        }
        return next;
      });
    }

    loadThumbnails();

    return () => {
      isCancelled = true;
    };
  }, [thumbUrls, timelineEntries]);

  async function handleSaveConfig() {
    setBusy(true);
    try {
      const saved = await saveConfig(config);
      setConfig(saved);
      setMessage("设置已保存");
      await refresh();
    } catch (error) {
      setMessage(formatError(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleRecordOnce() {
    setBusy(true);
    try {
      await recordOnce();
      await refresh();
    } catch (error) {
      setMessage(formatError(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleStart() {
    setBusy(true);
    try {
      setStatus(await startRecording());
    } catch (error) {
      setMessage(formatError(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleStop() {
    setBusy(true);
    try {
      setStatus(await stopRecording());
    } catch (error) {
      setMessage(formatError(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleRecordingToggle() {
    if (status?.is_recording) {
      await handleStop();
      return;
    }

    await handleStart();
  }

  async function handleSummary() {
    setBusy(true);
    try {
      const path = await generateSummary();
      setSummaryBlocks(await getTodaySummaryBlocks());
      setMessage(`日报已生成：${path}`);
    } catch (error) {
      setMessage(formatError(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="app-frame">
      <aside className="tab-rail" aria-label="主导航">
        <div className="brand">
          <img className="brand-mark" src={appIcon} alt="" aria-hidden="true" />
          <div>
            <h1>MindBack</h1>
          </div>
        </div>

        <nav className="tabs">
          <Button
            className={activeTab === "home" ? "tab active" : "tab"}
            variant="plain"
            onClick={() => setActiveTab("home")}
            aria-current={activeTab === "home" ? "page" : undefined}
            title="首页"
          >
            <House size={22} weight="regular" aria-hidden="true" />
            <span>首页</span>
          </Button>
          <Button
            className={activeTab === "settings" ? "tab active" : "tab"}
            variant="plain"
            onClick={() => setActiveTab("settings")}
            aria-current={activeTab === "settings" ? "page" : undefined}
            title="设置"
          >
            <SlidersHorizontal size={22} weight="regular" aria-hidden="true" />
            <span>设置</span>
          </Button>
        </nav>

        <div className="rail-footer">
          <span className={status?.is_recording ? "record-dot active" : "record-dot"} />
          <div>
            <strong>{status?.is_recording ? "记录中" : "未记录"}</strong>
            <span>{status?.today ?? "等待状态"}</span>
          </div>
        </div>
      </aside>

      <section className="workspace">
        {activeTab === "home" ? (
          <>
            <header className="workspace-header">
              <div>
                <h2>{config.project_name || "尚未设置今日项目"}</h2>
              </div>
              <div className="header-actions">
                <div className="header-stats" aria-label="今日快速统计">
                  <span>
                    <strong>{entries.length}</strong>
                    记录
                  </span>
                  <span>
                    <strong>{onProjectRatio}</strong>
                    符合
                  </span>
                </div>
                <Button
                  className={status?.is_recording ? "record-toggle recording" : "record-toggle"}
                  onClick={handleRecordingToggle}
                  disabled={isBusy}
                >
                  {status?.is_recording ? "停止记录" : "开始记录"}
                </Button>
              </div>
            </header>

            <div className="home-grid">
              <section className="panel timeline-panel" aria-labelledby="timeline-title">
                <div className="panel-heading">
                  <div>
                    <span className="eyebrow">日志追踪</span>
                    <h3 id="timeline-title">今日记录</h3>
                  </div>
                  <Button onClick={handleRecordOnce} disabled={isBusy}>
                    记录一次
                  </Button>
                </div>
                {entries.length === 0 ? (
                  <div className="empty-state">
                    <img className="empty-icon" src={appIcon} alt="" aria-hidden="true" />
                    <strong>还没有记录</strong>
                    <span>点击“记录一次”或开始后台记录后，这里会出现截图预览和时间线。</span>
                  </div>
                ) : (
                  <div className="capture-browser">
                    <div className="bucket-heading">
                      <strong>{selectedBucket?.range}</strong>
                      <span>{visibleEntries.length} 张截图</span>
                    </div>

                    <div className="capture-grid" aria-label="当前时间段截图预览">
                      {visibleEntries.length === 0 ? (
                        <div className="capture-empty">这个时间段没有截图记录。</div>
                      ) : (
                        visibleEntries.map((entry) => {
                          const thumbUrl = thumbUrls[entry.screenshot_thumb];

                          return (
                            <Button
                              className="capture-tile"
                              variant="plain"
                              key={`${entry.timestamp}-${entry.screenshot_thumb}`}
                              onClick={() => setDetailEntry(entry)}
                              aria-label={`查看 ${formatEntryTime(entry.timestamp)} 的识别结果`}
                            >
                              {thumbUrl ? (
                                <img src={thumbUrl} alt="" aria-hidden="true" />
                              ) : (
                                <span className="capture-placeholder" aria-hidden="true" />
                              )}
                              <span className="capture-time">
                                {formatEntryTime(entry.timestamp)}
                              </span>
                              <span
                                className={entry.is_on_project ? "status-dot good" : "status-dot warn"}
                                aria-hidden="true"
                              />
                            </Button>
                          );
                        })
                      )}
                    </div>

                    <div className="timeline-picker">
                      <div className="timeline-rule" aria-hidden="true">
                        {timelineBuckets.map((bucket, index) => (
                          <Button
                            className={
                              index === selectedBucketIndex ? "timeline-tick selected" : "timeline-tick"
                            }
                            variant="plain"
                            key={`${bucket.key}-tick`}
                            onClick={() => setSelectedBucketKey(bucket.key)}
                            aria-label={`选择 ${bucket.range} 的记录`}
                          />
                        ))}
                      </div>
                      <div className="timeline-labels" aria-hidden="true">
                        <span>{timelineBuckets[0]?.range}</span>
                        <strong>{selectedBucket?.range}</strong>
                        <span>
                          {timelineBuckets[timelineBuckets.length - 1]?.range}
                        </span>
                      </div>
                    </div>
                  </div>
                )}
              </section>

              <aside className="panel summary-panel" aria-labelledby="summary-title">
                <Tabs.Root
                  className="summary-tabs"
                  value={summaryPanelTab}
                  onValueChange={(value) =>
                    setSummaryPanelTab(value as SummaryPanelTab)
                  }
                >
                  <div className="panel-heading summary-heading">
                    <div>
                      <span className="eyebrow">今日概要</span>
                      <h3 id="summary-title">
                        {summaryPanelTab === "blocks" ? "时间段摘要" : "日报"}
                      </h3>
                    </div>
                    <Tabs.List className="summary-tab-list" aria-label="今日概要视图">
                      <Tabs.Tab className="summary-tab" value="blocks">
                        时间段
                      </Tabs.Tab>
                      <Tabs.Tab className="summary-tab" value="daily">
                        日报
                      </Tabs.Tab>
                    </Tabs.List>
                  </div>

                  <Tabs.Panel className="summary-tab-panel" value="blocks">
                    {summarySections.length === 0 ? (
                      <div className="summary-empty">
                        <strong>等待摘要</strong>
                        <p>开始记录后，这里会按时间段汇总你在做什么。</p>
                      </div>
                    ) : (
                      <div className="summary-list">
                        {summarySections.map((section) => (
                          <article className="summary-entry" key={section.range}>
                            <time>{section.range}</time>
                            <strong>{section.title}</strong>
                            <p>{section.detail}</p>
                          </article>
                        ))}
                      </div>
                    )}
                  </Tabs.Panel>

                  <Tabs.Panel className="summary-tab-panel" value="daily">
                    <div className="daily-report">
                      <section className="daily-report-hero">
                        <span>今日结论</span>
                        <strong>{dailyReportView.alignmentLabel}</strong>
                        <p>{dailyReportView.overview}</p>
                      </section>

                      <section className="daily-report-section">
                        <h4>完成了什么</h4>
                        <ul>
                          {dailyReportView.primaryWork.map((item) => (
                            <li key={item}>{item}</li>
                          ))}
                        </ul>
                      </section>

                      <section className="daily-report-section">
                        <h4>主要偏离</h4>
                        <ul>
                          {dailyReportView.driftNotes.map((item) => (
                            <li key={item}>{item}</li>
                          ))}
                        </ul>
                      </section>

                      <section className="daily-report-section">
                        <h4>复盘问题</h4>
                        <ul>
                          {dailyReportView.prompts.map((item) => (
                            <li key={item}>{item}</li>
                          ))}
                        </ul>
                      </section>
                    </div>
                  </Tabs.Panel>

                  <div className="summary-actions">
                    <Button onClick={handleSummary} disabled={isBusy}>
                      {summaryPanelTab === "daily" ? "重新生成" : "生成日报"}
                    </Button>
                    <Button
                      variant="secondary"
                      onClick={() => setActiveTab("settings")}
                    >
                      打开设置
                    </Button>
                  </div>
                  {message ? <p className="message">{message}</p> : null}
                </Tabs.Root>
              </aside>
            </div>

            {detailEntry ? (
              <Dialog.Root
                open
                onOpenChange={(open) => {
                  if (!open) {
                    setDetailEntry(null);
                  }
                }}
              >
                <Dialog.Portal>
                  <Dialog.Backdrop className="entry-modal-backdrop" />
                  <Dialog.Viewport className="entry-modal">
                    <Dialog.Popup className="entry-modal-panel">
                      <div className="entry-modal-heading">
                        <div>
                          <span className="eyebrow">模型识别结果</span>
                          <Dialog.Title id="entry-modal-title" render={<h3 />}>
                            {formatEntryTime(detailEntry.timestamp)}
                          </Dialog.Title>
                        </div>
                        <Dialog.Close className="ui-button ui-button-secondary">
                          关闭
                        </Dialog.Close>
                      </div>
                      <div className="entry-modal-body">
                        <div className="entry-preview">
                          {detailThumbUrl ? (
                            <img src={detailThumbUrl} alt="" />
                          ) : (
                            <span className="capture-placeholder" aria-hidden="true" />
                          )}
                        </div>
                        <div className="entry-result">
                          <span>Summary</span>
                          <strong>{detailEntry.intent}</strong>
                          <p>{detailEntry.reason}</p>
                          <small>{detailEntry.visible_context}</small>
                          <div className={detailEntry.is_on_project ? "badge good" : "badge warn"}>
                            {detailEntry.is_on_project ? "符合" : "偏离"} ·{" "}
                            {Math.round(detailEntry.confidence * 100)}%
                          </div>
                        </div>
                      </div>
                    </Dialog.Popup>
                  </Dialog.Viewport>
                </Dialog.Portal>
              </Dialog.Root>
            ) : null}
          </>
        ) : (
          <>
            <header className="workspace-header compact">
              <div>
                <span className="eyebrow">设置</span>
                <h2>设置</h2>
                <p>配置今日项目、截图间隔和本地识别模型。</p>
              </div>
            </header>

            <section className="panel settings-panel" aria-labelledby="settings-title">
              <div className="panel-heading">
                <div>
                  <span className="eyebrow">今日项目</span>
                  <h3 id="settings-title">监督配置</h3>
                </div>
                <Button onClick={handleSaveConfig} disabled={isBusy}>
                  保存设置
                </Button>
              </div>
              <div className="settings-grid">
                <Field.Root>
                  <Field.Label>名称</Field.Label>
                  <Input
                    value={config.project_name}
                    onValueChange={(value) =>
                      setConfig({ ...config, project_name: value })
                    }
                    placeholder="例如：MindBack MVP"
                  />
                </Field.Root>
                <Field.Root>
                  <Field.Label>截图间隔</Field.Label>
                  <Select
                    ariaLabel="截图间隔"
                    value={String(config.interval_seconds)}
                    onValueChange={(value) =>
                      setConfig({
                        ...config,
                        interval_seconds: Number(value),
                      })
                    }
                    options={INTERVAL_OPTIONS}
                  />
                </Field.Root>
                <Field.Root className="wide">
                  <Field.Label>描述</Field.Label>
                  <Textarea
                    value={config.project_description}
                    onValueChange={(value) =>
                      setConfig({
                        ...config,
                        project_description: value,
                      })
                    }
                    placeholder="今天只围绕这个项目推进。"
                  />
                </Field.Root>
                <Field.Root className="wide">
                  <Field.Label>模型</Field.Label>
                  <Select
                    ariaLabel="模型"
                    value={config.model}
                    onValueChange={(value) =>
                      setConfig({ ...config, model: value })
                    }
                    options={MODEL_OPTIONS}
                  />
                </Field.Root>
              </div>
              {message ? <p className="message">{message}</p> : null}
            </section>
          </>
        )}
      </section>
    </main>
  );
}

export default App;
