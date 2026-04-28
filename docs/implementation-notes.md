# MindBack 实施记录

日期：2026-04-28

## 当前状态

已完成第一条可运行的本地 MVP 骨架：

- Tauri v2 + React + Vite + TypeScript + Bun 项目已搭建。
- Rust 后端已提供配置、状态、日志读取、记录一次、生成日报命令。
- 本地数据目录使用 `~/Library/Application Support/MindBack`。
- `record_once` 目前使用确定性模拟截图生成清晰 JPEG 缩略图，并写入 `log.jsonl`。
- 识别层已抽象为 `RecognitionService`，当前返回确定性结果。
- `workers/mlx_worker.py` 定义了 MLX-VLM worker CLI 契约，并在缺少模型或图片时返回结构化 JSON 错误。

## 当前限制

- 真实 macOS 截图还未接入。下一步应验证 `tauri-plugin-screenshots` 或实现 macOS 专用 bridge。
- Rust 后端还没有调用 Python worker。当前先保留 worker 契约和确定性识别结果，便于 UI 与日志闭环稳定验证。
- 菜单栏入口还未实现。当前主窗口已经可用于配置和手动记录一次。
- 真实 `mlx-community/Qwen3-VL-8B-Instruct-4bit` 与 `mlx-community/gemma-4-e4b-it-4bit` 模型 smoke test 尚未执行。

## 已验证命令

```bash
bun run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

以上命令已在本地通过。

