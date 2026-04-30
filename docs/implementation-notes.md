# MindBack 实施记录

日期：2026-04-28

## 当前状态

已完成第一条可运行的本地 MVP 骨架：

- Tauri v2 + React + Vite + TypeScript + Bun 项目已搭建。
- Rust 后端已提供配置、状态、日志读取、记录一次、生成日报命令。
- 本地数据目录使用 `~/Library/Application Support/MindBack`。
- `record_once` 会调用 macOS `/usr/sbin/screencapture` 截取当前屏幕，并生成宽度不超过 1280px 的 JPEG 缩略图写入 `log.jsonl`。
- 后台自动记录会先检查主显示器是否在线、激活且未息屏；不可截图时跳过本轮，不写入日志。
- 识别层已抽象为 `RecognitionService`，默认调用本地 MLX-VLM worker。
- `workers/mlx_worker.py` 定义了 MLX-VLM worker CLI 契约，并在缺少 `mlx-vlm`、模型或图片时返回结构化 JSON 错误。
- 总结层已设计为 Vercel AI SDK Summary Agent。右侧时间段摘要只总结刚结束的前半小时窗口，并按窗口缓存；当前窗口继续使用本地规则摘要。
- `workers/summary_agent.ts` 定义了 Bun sidecar 契约，使用 `ai`、`@ai-sdk/deepseek` 和 `zod`。缺少 `DEEPSEEK_API_KEY` 时返回结构化错误，Rust 会回退到本地摘要。

## 当前限制

- 截图当前依赖 macOS `screencapture` 命令。后续可以评估 `tauri-plugin-screenshots` 或实现 macOS 专用 bridge，以获得更细的权限和多屏控制。
- Rust 后端默认调用 Python worker。默认 Python 优先使用 `~/Library/Application Support/MindBack/venvs/mlx-worker/bin/python`，不存在时回退到 `python3`；模型路径仍可通过设置页选择的模型名或本地模型路径解析。
- Summary Agent 默认模型为 `deepseek-v4-flash` 的非思考模式，需要 `DEEPSEEK_API_KEY`。未配置 key 时，半小时摘要和日报仍会用本地规则生成。
- 本地开发可复制 `.env.example` 为 `.env` 后填入 `DEEPSEEK_API_KEY`，或在启动前执行 `export DEEPSEEK_API_KEY=...`。真实 `.env` 已被 `.gitignore` 排除，不应提交。
- Summary Agent 第一版只接收每张截图已有的文本识别 summary，不上传原始截图或全天图片。
- 菜单栏入口还未实现。当前主窗口已经可用于配置和手动记录一次。
- 真实 `mlx-community/Qwen3-VL-8B-Instruct-4bit` 与 `mlx-community/gemma-4-e4b-it-4bit` 模型 smoke test 尚未执行。

## 已验证命令

```bash
bun run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

以上命令已在本地通过。

`bun run tauri dev` 已成功启动 Vite dev server 和 Tauri 桌面进程。
