# MindBack MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first runnable MindBack macOS MVP: a Tauri app that stores a daily project, records supervision log entries, exposes a simple UI, and provides the backend seams for screenshot capture and MLX-VLM recognition.

**Architecture:** Use a Tauri v2 app with React/Vite for UI and Rust for local state, storage, and commands. Keep MLX recognition behind a narrow worker interface so the first implementation can verify the app/logging flow before bundling heavy model dependencies.

**Tech Stack:** Tauri v2, React, Vite, TypeScript, Bun, Rust, serde, chrono, image, Python `mlx-vlm` worker script.

---

## File Structure

- Create `package.json`: Bun scripts and frontend dependencies.
- Create `index.html`: Vite app entry.
- Create `tsconfig.json`, `tsconfig.node.json`, `vite.config.ts`: TypeScript and Vite configuration.
- Create `src/main.tsx`: React bootstrap.
- Create `src/App.tsx`: Main UI shell for project setup, recording controls, and timeline.
- Create `src/styles.css`: Quiet, utilitarian desktop UI styling.
- Create `src/lib/api.ts`: Typed Tauri command wrappers.
- Create `src/lib/types.ts`: Shared frontend types matching Rust command payloads.
- Create `src-tauri/Cargo.toml`: Rust package and Tauri dependencies.
- Create `src-tauri/build.rs`: Tauri build hook.
- Create `src-tauri/tauri.conf.json`: Tauri app configuration for MindBack.
- Create `src-tauri/src/lib.rs`: Tauri setup and command registration.
- Create `src-tauri/src/main.rs`: Desktop entrypoint.
- Create `src-tauri/src/app_state.rs`: Shared app state and storage path setup.
- Create `src-tauri/src/commands.rs`: Commands called by React.
- Create `src-tauri/src/models.rs`: Serializable config/log/domain types.
- Create `src-tauri/src/storage.rs`: JSON config, JSONL log, Markdown summary persistence.
- Create `src-tauri/src/recorder.rs`: Recording session state and periodic tick orchestration.
- Create `src-tauri/src/capture.rs`: Screenshot interface, with a deterministic placeholder capture for the first smoke path.
- Create `src-tauri/src/recognition.rs`: Recognition interface, placeholder result, and future Python worker command boundary.
- Create `workers/mlx_worker.py`: CLI worker contract for future MLX-VLM recognition.
- Create `docs/implementation-notes.md`: Current MVP limitations and validation notes.

## Task 1: Scaffold Tauri and React Project

**Files:**
- Create: `package.json`
- Create: `index.html`
- Create: `tsconfig.json`
- Create: `tsconfig.node.json`
- Create: `vite.config.ts`
- Create: `src/main.tsx`
- Create: `src/App.tsx`
- Create: `src/styles.css`
- Create: `src/lib/api.ts`
- Create: `src/lib/types.ts`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/main.rs`

- [ ] **Step 1: Create frontend and Tauri config files**

Write the minimal Tauri + Vite + React files listed above. Use app name `MindBack`, bundle identifier `dev.mindback.app`, and product name `MindBack`.

- [ ] **Step 2: Install dependencies**

Run: `bun install`

Expected: dependencies install and `bun.lock` is created.

- [ ] **Step 3: Verify frontend typecheck/build**

Run: `bun run build`

Expected: Vite builds `dist/` successfully.

- [ ] **Step 4: Verify Rust shell compiles**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`

Expected: Rust compiles with the placeholder Tauri shell.

- [ ] **Step 5: Commit**

```bash
git add package.json bun.lock index.html tsconfig.json tsconfig.node.json vite.config.ts src src-tauri
git commit -m "feat: scaffold mindback tauri app"
```

## Task 2: Add Local Domain Model and Storage

**Files:**
- Create: `src-tauri/src/models.rs`
- Create: `src-tauri/src/storage.rs`
- Create: `src-tauri/src/app_state.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: Define serializable model types**

Create Rust types for `AppConfig`, `LogEntry`, `RecognitionResult`, `AppStatus`, and `RecordingState`. Use `serde` for JSON and `chrono` for timestamps.

- [ ] **Step 2: Implement storage paths**

Use `~/Library/Application Support/MindBack` on macOS through Tauri path APIs when available, with a safe fallback to the current directory in tests.

- [ ] **Step 3: Implement config read/write**

Commands:

- `get_config() -> AppConfig`
- `save_config(config: AppConfig) -> AppConfig`

Expected behavior: missing config returns defaults; save persists JSON.

- [ ] **Step 4: Implement JSONL append and daily log read**

Commands:

- `list_today_entries() -> Vec<LogEntry>`
- internal `append_log_entry(entry: &LogEntry)`

Expected behavior: daily directory is created as `days/YYYY-MM-DD/`, and logs append to `log.jsonl`.

- [ ] **Step 5: Verify Rust tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml storage`

Expected: config and JSONL tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src
git commit -m "feat: add local storage model"
```

## Task 3: Add Recording Tick with Placeholder Capture and Recognition

**Files:**
- Create: `src-tauri/src/recorder.rs`
- Create: `src-tauri/src/capture.rs`
- Create: `src-tauri/src/recognition.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Implement capture interface**

Define `CaptureService` with `capture_once(day_dir) -> CaptureResult`. First implementation writes a deterministic placeholder JPEG thumbnail under `thumbs/HH-MM-SS.jpg` so the full app flow can be tested before macOS screenshot permissions are wired.

- [ ] **Step 2: Implement recognition interface**

Define `RecognitionService` with `recognize(image_path, config) -> RecognitionResult`. First implementation returns a deterministic result that includes the configured project name and `is_on_project: true`.

- [ ] **Step 3: Implement one-shot recording command**

Command:

- `record_once() -> LogEntry`

Expected behavior: creates a thumbnail, creates a recognition result, appends one JSONL row, and returns it to the UI.

- [ ] **Step 4: Implement start/stop status**

Commands:

- `start_recording() -> AppStatus`
- `stop_recording() -> AppStatus`
- `get_status() -> AppStatus`

First implementation can keep the session state and expose manual `record_once`; the periodic timer is added after one-shot is stable.

- [ ] **Step 5: Verify one-shot flow**

Run: `cargo test --manifest-path src-tauri/Cargo.toml recorder`

Expected: one-shot record creates a JSONL entry and thumbnail path.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src src-tauri/Cargo.toml
git commit -m "feat: add recording log flow"
```

## Task 4: Wire React UI to Commands

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/api.ts`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Add frontend types and API wrappers**

Mirror Rust payloads in TypeScript and wrap `invoke` calls for config, status, entries, and `record_once`.

- [ ] **Step 2: Build the main screen**

Implement UI sections:

- Today project config.
- Recording state controls.
- Manual “record once” button for MVP verification.
- Timeline list with timestamp, thumbnail, intent, confidence, and reason.

- [ ] **Step 3: Build settings controls**

Expose interval and model selection with the two MVP models:

- `mlx-community/Qwen3-VL-8B-Instruct-4bit`
- `mlx-community/gemma-4-e4b-it-4bit`

- [ ] **Step 4: Verify frontend build**

Run: `bun run build`

Expected: TypeScript and Vite build succeed.

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "feat: wire mindback ui"
```

## Task 5: Add MLX Worker Contract

**Files:**
- Create: `workers/mlx_worker.py`
- Modify: `src-tauri/src/recognition.rs`
- Create: `docs/implementation-notes.md`

- [ ] **Step 1: Create Python worker CLI**

The worker accepts:

```bash
python workers/mlx_worker.py --model <model_id> --project <project> --image <image_path>
```

It prints JSON with `intent`, `is_on_project`, `confidence`, `reason`, and `visible_context`.

- [ ] **Step 2: Implement dry-run mode**

If `mlx-vlm` is unavailable, the worker returns a clear JSON error with `worker_unavailable`. This keeps local development testable before installing the model.

- [ ] **Step 3: Document current limitation**

Write `docs/implementation-notes.md` stating that the MVP currently has a placeholder capture path and a worker contract, while real ScreenCaptureKit/plugin integration and actual MLX model smoke tests are next.

- [ ] **Step 4: Verify worker dry run**

Run: `python3 workers/mlx_worker.py --model mlx-community/Qwen3-VL-8B-Instruct-4bit --project MindBack --image /tmp/missing.jpg`

Expected: JSON error, not a Python stack trace.

- [ ] **Step 5: Commit**

```bash
git add workers docs/implementation-notes.md src-tauri/src/recognition.rs
git commit -m "feat: add mlx worker contract"
```

## Task 6: End-to-End Smoke

**Files:**
- Modify as needed: `README.md`
- Modify as needed: `docs/implementation-notes.md`

- [ ] **Step 1: Run validation commands**

Run:

```bash
bun run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: all pass.

- [ ] **Step 2: Start dev app**

Run: `bun run tauri dev`

Expected: MindBack opens, config saves, record-once creates a timeline item.

- [ ] **Step 3: Update docs with actual validation result**

Record what passed and what remains manual in `docs/implementation-notes.md`.

- [ ] **Step 4: Commit**

```bash
git add README.md docs/implementation-notes.md
git commit -m "docs: add mindback mvp validation notes"
```

## Self-Review

- Spec coverage: The plan covers naming, Tauri/React/Bun app shape, local storage, JSONL/Markdown log foundation, model selection, and MLX worker boundary. Real screenshot capture is intentionally separated after placeholder flow so the app can become runnable before permission-heavy native capture work.
- Placeholder scan: The implementation contains a deliberate placeholder capture service in Task 3, documented as an MVP stepping stone, not an unspecified requirement.
- Type consistency: Rust and TypeScript share the same domain names: `AppConfig`, `LogEntry`, `RecognitionResult`, and `AppStatus`.

