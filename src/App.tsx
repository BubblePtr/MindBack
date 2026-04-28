import { useEffect, useMemo, useState } from "react";
import {
  generateSummary,
  getConfig,
  getStatus,
  listTodayEntries,
  recordOnce,
  saveConfig,
  startRecording,
  stopRecording,
} from "./lib/api";
import type { AppConfig, AppStatus, LogEntry } from "./lib/types";

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

function App() {
  const [config, setConfig] = useState<AppConfig>(DEFAULT_CONFIG);
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [message, setMessage] = useState<string>("");
  const [isBusy, setBusy] = useState(false);

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
    refresh().catch((error) => setMessage(String(error)));
  }, []);

  async function handleSaveConfig() {
    setBusy(true);
    try {
      const saved = await saveConfig(config);
      setConfig(saved);
      setMessage("设置已保存");
      await refresh();
    } catch (error) {
      setMessage(String(error));
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
      setMessage(String(error));
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
      setMessage(String(error));
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
      setMessage(String(error));
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
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="app-shell">
      <header className="topbar">
        <div>
          <h1>MindBack</h1>
          <p>回神 · 把注意力带回今日项目</p>
        </div>
        <div className={status?.is_recording ? "status active" : "status"}>
          {status?.is_recording ? "记录中" : "未记录"}
        </div>
      </header>

      <section className="layout">
        <aside className="panel settings-panel">
          <h2>今日项目</h2>
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
          <label>
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
          <div className="button-row">
            <button onClick={handleSaveConfig} disabled={isBusy}>
              保存设置
            </button>
          </div>
        </aside>

        <section className="content">
          <div className="panel summary-panel">
            <div>
              <span className="eyebrow">今日状态</span>
              <h2>{config.project_name || "尚未设置今日项目"}</h2>
              <p>{config.project_description || "设置今日项目后开始记录。"}</p>
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
                <dt>日期</dt>
                <dd>{status?.today ?? "-"}</dd>
              </div>
            </dl>
            <div className="button-row">
              <button onClick={handleStart} disabled={isBusy}>
                开始记录
              </button>
              <button onClick={handleStop} disabled={isBusy}>
                停止记录
              </button>
              <button onClick={handleRecordOnce} disabled={isBusy}>
                记录一次
              </button>
              <button onClick={handleSummary} disabled={isBusy}>
                生成日报
              </button>
            </div>
            {message ? <p className="message">{message}</p> : null}
          </div>

          <div className="panel timeline-panel">
            <h2>监督日志</h2>
            {entries.length === 0 ? (
              <div className="empty-state">还没有记录。</div>
            ) : (
              <ol className="timeline">
                {entries
                  .slice()
                  .reverse()
                  .map((entry) => (
                    <li key={`${entry.timestamp}-${entry.screenshot_thumb}`}>
                      <div className="timeline-main">
                        <time>{new Date(entry.timestamp).toLocaleString()}</time>
                        <strong>{entry.intent}</strong>
                        <p>{entry.reason}</p>
                        <span>{entry.visible_context}</span>
                      </div>
                      <div
                        className={
                          entry.is_on_project ? "badge good" : "badge warn"
                        }
                      >
                        {entry.is_on_project ? "符合" : "偏离"} ·{" "}
                        {Math.round(entry.confidence * 100)}%
                      </div>
                    </li>
                  ))}
              </ol>
            )}
          </div>
        </section>
      </section>
    </main>
  );
}

export default App;
