import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { House, SlidersHorizontal } from "@phosphor-icons/react";
import {
  generateSummary,
  getConfig,
  getStatus,
  getTodayThumbnail,
  listTodayEntries,
  recordOnce,
  saveConfig,
  startRecording,
  stopRecording,
} from "./lib/api";
import type { AppConfig, AppStatus, LogEntry } from "./lib/types";
import appIcon from "./assets/mindback-app-icon.png";

const MODELS = [
  "mlx-community/Qwen3-VL-4B-Instruct-4bit",
  "mlx-community/Qwen3-VL-8B-Instruct-4bit",
  "mlx-community/gemma-4-e4b-it-4bit",
];

const DEFAULT_CONFIG: AppConfig = {
  project_name: "",
  project_description: "",
  interval_seconds: 60,
  model: MODELS[0],
};

type ActiveTab = "home" | "settings";
type TimelineBucket = {
  key: string;
  range: string;
  entries: LogEntry[];
  title: string;
  detail: string;
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

function formatBucketRange(bucketKey: string) {
  const start = new Date(bucketKey);
  const end = new Date(start);
  end.setMinutes(start.getMinutes() + 30);
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

function App() {
  const [activeTab, setActiveTab] = useState<ActiveTab>("home");
  const [config, setConfig] = useState<AppConfig>(DEFAULT_CONFIG);
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [selectedBucketKey, setSelectedBucketKey] = useState<string | null>(null);
  const [detailEntry, setDetailEntry] = useState<LogEntry | null>(null);
  const [thumbUrls, setThumbUrls] = useState<Record<string, string>>({});
  const [message, setMessage] = useState<string>("");
  const [isBusy, setBusy] = useState(false);
  const activeTabRef = useRef(activeTab);
  const refreshInFlightRef = useRef(false);

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

    return [...buckets.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, bucketEntries]) => summarizeBucket(key, bucketEntries));
  }, [timelineEntries]);

  const selectedBucketIndex = useMemo(() => {
    if (timelineBuckets.length === 0) return 0;
    const index = timelineBuckets.findIndex((bucket) => bucket.key === selectedBucketKey);
    return index >= 0 ? index : timelineBuckets.length - 1;
  }, [selectedBucketKey, timelineBuckets]);

  const selectedBucket = timelineBuckets[selectedBucketIndex] ?? null;
  const visibleEntries = selectedBucket?.entries ?? [];

  const onProjectRatio = useMemo(() => {
    if (entries.length === 0) return "0%";
    const onProject = entries.filter((entry) => entry.is_on_project).length;
    return `${Math.round((onProject / entries.length) * 100)}%`;
  }, [entries]);

  const detailThumbUrl = detailEntry ? thumbUrls[detailEntry.screenshot_thumb] : "";

  useEffect(() => {
    activeTabRef.current = activeTab;
  }, [activeTab]);

  const refresh = useCallback(async (options: { syncConfig?: boolean } = {}) => {
    const syncConfig = options.syncConfig ?? activeTabRef.current !== "settings";
    const [nextConfig, nextStatus, nextEntries] = await Promise.all([
      getConfig(),
      getStatus(),
      listTodayEntries(),
    ]);
    if (syncConfig) {
      setConfig(nextConfig);
    }
    setStatus(nextStatus);
    setEntries(nextEntries);
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
          <button
            className={activeTab === "home" ? "tab active" : "tab"}
            type="button"
            onClick={() => setActiveTab("home")}
            aria-current={activeTab === "home" ? "page" : undefined}
            title="首页"
          >
            <House size={22} weight="regular" aria-hidden="true" />
            <span>首页</span>
          </button>
          <button
            className={activeTab === "settings" ? "tab active" : "tab"}
            type="button"
            onClick={() => setActiveTab("settings")}
            aria-current={activeTab === "settings" ? "page" : undefined}
            title="设置"
          >
            <SlidersHorizontal size={22} weight="regular" aria-hidden="true" />
            <span>设置</span>
          </button>
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
                <button
                  className={status?.is_recording ? "record-toggle recording" : "record-toggle"}
                  onClick={handleRecordingToggle}
                  disabled={isBusy}
                  type="button"
                >
                  {status?.is_recording ? "停止记录" : "开始记录"}
                </button>
              </div>
            </header>

            <div className="home-grid">
              <section className="panel timeline-panel" aria-labelledby="timeline-title">
                <div className="panel-heading">
                  <div>
                    <span className="eyebrow">日志追踪</span>
                    <h3 id="timeline-title">今日记录</h3>
                  </div>
                  <button onClick={handleRecordOnce} disabled={isBusy} type="button">
                    记录一次
                  </button>
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
                      {visibleEntries.map((entry) => {
                        const thumbUrl = thumbUrls[entry.screenshot_thumb];

                        return (
                          <button
                            className="capture-tile"
                            key={`${entry.timestamp}-${entry.screenshot_thumb}`}
                            type="button"
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
                          </button>
                        );
                      })}
                    </div>

                    <div className="timeline-picker">
                      <div className="timeline-rule" aria-hidden="true">
                        {timelineBuckets.map((bucket, index) => (
                          <button
                            className={
                              index === selectedBucketIndex ? "timeline-tick selected" : "timeline-tick"
                            }
                            key={`${bucket.key}-tick`}
                            type="button"
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
                <div className="panel-heading">
                  <div>
                    <span className="eyebrow">今日概要</span>
                    <h3 id="summary-title">时间段摘要</h3>
                  </div>
                </div>
                {timelineBuckets.length === 0 ? (
                  <div className="summary-empty">
                    <strong>等待摘要</strong>
                    <p>开始记录后，这里会按时间段汇总你在做什么。</p>
                  </div>
                ) : (
                  <div className="summary-list">
                    {timelineBuckets.map((section) => (
                      <article className="summary-entry" key={section.range}>
                        <time>{section.range}</time>
                        <strong>{section.title}</strong>
                        <p>{section.detail}</p>
                      </article>
                    ))}
                  </div>
                )}
                <div className="summary-actions">
                  <button onClick={handleSummary} disabled={isBusy} type="button">
                    生成日报
                  </button>
                  <button
                    className="secondary"
                    onClick={() => setActiveTab("settings")}
                    type="button"
                  >
                    打开设置
                  </button>
                </div>
                {message ? <p className="message">{message}</p> : null}
              </aside>
            </div>

            {detailEntry ? (
              <div className="entry-modal" role="dialog" aria-modal="true" aria-labelledby="entry-modal-title">
                <button
                  className="entry-modal-backdrop"
                  type="button"
                  aria-label="关闭识别结果"
                  onClick={() => setDetailEntry(null)}
                />
                <section className="entry-modal-panel">
                  <div className="entry-modal-heading">
                    <div>
                      <span className="eyebrow">模型识别结果</span>
                      <h3 id="entry-modal-title">{formatEntryTime(detailEntry.timestamp)}</h3>
                    </div>
                    <button
                      className="secondary"
                      type="button"
                      onClick={() => setDetailEntry(null)}
                    >
                      关闭
                    </button>
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
                </section>
              </div>
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
                <button onClick={handleSaveConfig} disabled={isBusy} type="button">
                  保存设置
                </button>
              </div>
              <div className="settings-grid">
                <label>
                  名称
                  <input
                    value={config.project_name}
                    onChange={(event) =>
                      setConfig({ ...config, project_name: event.target.value })
                    }
                    placeholder="例如：MindBack MVP"
                  />
                </label>
                <label>
                  截图间隔
                  <select
                    value={config.interval_seconds}
                    onChange={(event) =>
                      setConfig({
                        ...config,
                        interval_seconds: Number(event.target.value),
                      })
                    }
                  >
                    {[30, 60, 120, 300].map((seconds) => (
                      <option key={seconds} value={seconds}>
                        {seconds} 秒
                      </option>
                    ))}
                  </select>
                </label>
                <label className="wide">
                  描述
                  <textarea
                    value={config.project_description}
                    onChange={(event) =>
                      setConfig({
                        ...config,
                        project_description: event.target.value,
                      })
                    }
                    placeholder="今天只围绕这个项目推进。"
                  />
                </label>
                <label className="wide">
                  模型
                  <select
                    value={config.model}
                    onChange={(event) =>
                      setConfig({ ...config, model: event.target.value })
                    }
                  >
                    {MODELS.map((model) => (
                      <option key={model} value={model}>
                        {model}
                      </option>
                    ))}
                  </select>
                </label>
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
