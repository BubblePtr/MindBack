import { useEffect, useMemo, useState } from "react";
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

function formatEntryTime(timestamp: string) {
  return new Date(timestamp).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatError(error: unknown) {
  const text = String(error);
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
  const [selectedTimestamp, setSelectedTimestamp] = useState<string | null>(null);
  const [thumbUrls, setThumbUrls] = useState<Record<string, string>>({});
  const [message, setMessage] = useState<string>("");
  const [isBusy, setBusy] = useState(false);

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

  const selectedIndex = useMemo(() => {
    if (timelineEntries.length === 0) return 0;
    const index = timelineEntries.findIndex(
      (entry) => entry.timestamp === selectedTimestamp,
    );
    return index >= 0 ? index : timelineEntries.length - 1;
  }, [selectedTimestamp, timelineEntries]);

  const selectedEntry = timelineEntries[selectedIndex] ?? null;

  const onProjectRatio = useMemo(() => {
    if (entries.length === 0) return "0%";
    const onProject = entries.filter((entry) => entry.is_on_project).length;
    return `${Math.round((onProject / entries.length) * 100)}%`;
  }, [entries]);

  async function refresh() {
    const [nextConfig, nextStatus, nextEntries] = await Promise.all([
      getConfig(),
      getStatus(),
      listTodayEntries(),
    ]);
    setConfig(nextConfig);
    setStatus(nextStatus);
    setEntries(nextEntries);
  }

  useEffect(() => {
    refresh().catch((error) => setMessage(formatError(error)));
  }, []);

  useEffect(() => {
    if (timelineEntries.length === 0) {
      setSelectedTimestamp(null);
      return;
    }

    if (
      selectedTimestamp === null ||
      !timelineEntries.some((entry) => entry.timestamp === selectedTimestamp)
    ) {
      setSelectedTimestamp(timelineEntries[timelineEntries.length - 1].timestamp);
    }
  }, [selectedTimestamp, timelineEntries]);

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
      setMessage("已写入一条监督记录");
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
      setMessage("记录已开始");
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
      setMessage("记录已停止");
    } catch (error) {
      setMessage(formatError(error));
    } finally {
      setBusy(false);
    }
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
            <p>回神</p>
          </div>
        </div>

        <nav className="tabs">
          <button
            className={activeTab === "home" ? "tab active" : "tab"}
            type="button"
            onClick={() => setActiveTab("home")}
            aria-current={activeTab === "home" ? "page" : undefined}
            title="Home"
          >
            <House size={22} weight="regular" aria-hidden="true" />
            <span>Home</span>
          </button>
          <button
            className={activeTab === "settings" ? "tab active" : "tab"}
            type="button"
            onClick={() => setActiveTab("settings")}
            aria-current={activeTab === "settings" ? "page" : undefined}
            title="Settings"
          >
            <SlidersHorizontal size={22} weight="regular" aria-hidden="true" />
            <span>Settings</span>
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
                <span className="eyebrow">Home</span>
                <h2>{config.project_name || "尚未设置今日项目"}</h2>
                <p>{config.project_description || "设置今日项目后开始记录。"}</p>
              </div>
              <div className="header-actions">
                <button onClick={handleStart} disabled={isBusy} type="button">
                  开始记录
                </button>
                <button
                  className="secondary"
                  onClick={handleStop}
                  disabled={isBusy}
                  type="button"
                >
                  停止
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
                    <div className="capture-grid" aria-label="截图预览">
                      {timelineEntries.map((entry, index) => {
                        const isSelected = index === selectedIndex;
                        const thumbUrl = thumbUrls[entry.screenshot_thumb];

                        return (
                          <button
                            className={
                              isSelected ? "capture-tile selected" : "capture-tile"
                            }
                            key={`${entry.timestamp}-${entry.screenshot_thumb}`}
                            type="button"
                            onClick={() => setSelectedTimestamp(entry.timestamp)}
                            aria-pressed={isSelected}
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
                        {timelineEntries.map((entry, index) => (
                          <button
                            className={
                              index === selectedIndex ? "timeline-tick selected" : "timeline-tick"
                            }
                            key={`${entry.timestamp}-tick`}
                            type="button"
                            onClick={() => setSelectedTimestamp(entry.timestamp)}
                            aria-label={`选择 ${formatEntryTime(entry.timestamp)} 的记录`}
                          />
                        ))}
                      </div>
                      <input
                        aria-label="选择时间线记录"
                        className="timeline-range"
                        type="range"
                        min="0"
                        max={Math.max(timelineEntries.length - 1, 0)}
                        value={selectedIndex}
                        onChange={(event) =>
                          setSelectedTimestamp(
                            timelineEntries[Number(event.target.value)]?.timestamp ?? null,
                          )
                        }
                      />
                      <div className="timeline-labels" aria-hidden="true">
                        <span>{formatEntryTime(timelineEntries[0].timestamp)}</span>
                        <strong>{formatEntryTime(selectedEntry?.timestamp ?? "")}</strong>
                        <span>
                          {formatEntryTime(
                            timelineEntries[timelineEntries.length - 1].timestamp,
                          )}
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
                    <h3 id="summary-title">专注概览</h3>
                  </div>
                </div>
                <dl className="metrics">
                  <div>
                    <dt>记录数</dt>
                    <dd>{entries.length}</dd>
                  </div>
                  <div>
                    <dt>符合比例</dt>
                    <dd>{onProjectRatio}</dd>
                  </div>
                  <div>
                    <dt>截图间隔</dt>
                    <dd>{config.interval_seconds}s</dd>
                  </div>
                </dl>
                <div className="summary-card">
                  <span>选中记录</span>
                  {selectedEntry ? (
                    <>
                      <strong>{selectedEntry.intent}</strong>
                      <p>{selectedEntry.reason}</p>
                      <small>{selectedEntry.visible_context}</small>
                      <div
                        className={
                          selectedEntry.is_on_project ? "badge good" : "badge warn"
                        }
                      >
                        {selectedEntry.is_on_project ? "符合" : "偏离"} ·{" "}
                        {Math.round(selectedEntry.confidence * 100)}%
                      </div>
                    </>
                  ) : (
                    <strong>等待记录</strong>
                  )}
                </div>
                <div className="summary-card compact">
                  <span>当前模型</span>
                  <strong>{config.model}</strong>
                </div>
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
          </>
        ) : (
          <>
            <header className="workspace-header compact">
              <div>
                <span className="eyebrow">Settings</span>
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
