# Summary Agent Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Vercel AI SDK Summary Agent that summarizes only the previous completed half-hour for the home page, then reuses cached window summaries for daily Markdown reports.

**Architecture:** Rust owns storage, time-window bucketing, sidecar invocation, fallback summaries, and Markdown rendering. A Bun sidecar in `workers/summary_agent.ts` owns the AI SDK `ToolLoopAgent` call and returns validated JSON. React reads cached summary blocks and keeps the current local rule summary as a fallback.

**Tech Stack:** Tauri v2, Rust, serde, chrono, Bun, Vercel AI SDK v6, `@ai-sdk/deepseek`, Zod, React, TypeScript.

---

## File Structure

- Modify `package.json` and `bun.lock`: add `ai`, `@ai-sdk/deepseek`, and `zod`.
- Create `workers/summary_agent.ts`: Bun CLI sidecar that reads JSON from stdin and returns Summary Agent JSON.
- Modify `src-tauri/src/models.rs`: add summary config fields plus `SummaryTimeBlock`, `SummaryAgentResult`, and request structs.
- Create `src-tauri/src/summary.rs`: half-hour window bucketing, cache persistence, sidecar invocation, fallback summaries, and Markdown rendering.
- Modify `src-tauri/src/storage.rs`: expose day-log helpers and delegate summary writing to `summary.rs`.
- Modify `src-tauri/src/commands.rs`: add `get_today_summary_blocks` and `summarize_previous_half_hour`.
- Modify `src-tauri/src/lib.rs` and `src-tauri/src/dev_bridge.rs`: register commands and browser preview endpoints.
- Modify `src/lib/types.ts` and `src/lib/api.ts`: add typed summary block APIs.
- Modify `src/App.tsx`: render Agent-backed summary blocks when available, otherwise keep local rule summaries.
- Modify `docs/implementation-notes.md`: document Summary Agent requirements and fallbacks.

## Tasks

### Task 1: Rust Summary Domain and Window Behavior

- [ ] Add failing tests in `src-tauri/src/summary.rs` for 30-minute bucket boundaries, previous completed half-hour detection, empty-window fallback, cached block read/write, and sidecar failure fallback.
- [ ] Implement summary structs in `models.rs`.
- [ ] Implement `SummaryService` in `summary.rs` with local fallback first.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml summary`.

### Task 2: Sidecar Contract

- [ ] Add `workers/summary_agent.ts` with stdin parsing, Zod schemas, `ToolLoopAgent`, DeepSeek provider, and deterministic JSON error output when `DEEPSEEK_API_KEY` is missing.
- [ ] Add dependencies with `bun add ai @ai-sdk/deepseek zod`.
- [ ] Verify dry run without API key returns structured JSON error.

### Task 3: Tauri and Dev Bridge Integration

- [ ] Wire `SummaryService` into `generate_summary`, `get_today_summary_blocks`, and `summarize_previous_half_hour`.
- [ ] Register Tauri commands and dev bridge endpoints.
- [ ] Run targeted Rust tests for summary and dev bridge behavior.

### Task 4: React UI Integration

- [ ] Add TypeScript summary types and API wrappers.
- [ ] Fetch summary blocks during refresh without creating request waterfalls.
- [ ] Render right-side half-hour cards from cached Agent summaries when present; use existing local `summarizeBucket` output for current or unavailable windows.
- [ ] Run `bun run build`.

### Task 5: Verification and Docs

- [ ] Update `docs/implementation-notes.md`.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml summary`.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] Run `bun run build`.
- [ ] Run `git diff --check`.
