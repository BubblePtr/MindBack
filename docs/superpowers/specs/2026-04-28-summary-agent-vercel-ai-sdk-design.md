# MindBack 总结 Agent 设计

日期：2026-04-28

## 背景

MindBack 当前是一个偏记录型的本地监督日志工具。第一版已经围绕“今日项目”、定时截图识别、JSONL 日志、主页面右侧半小时摘要和 Markdown 日报形成基础闭环。新增总结 Agent 的目标不是把产品升级成复杂助理，而是让当天查看和晚上复盘更省力：从当天结构化记录中提炼出可信、克制、可追溯的时间段摘要和日报。

本设计确认使用 Vercel AI SDK v6 的 Agent 能力。工程上保持小边界：Rust 仍然负责本地存储、读取日志和写入日报；AI SDK sidecar 只负责把当天记录总结成结构化 JSON。

## 目标

- 使用 Vercel AI SDK v6 构建 Summary Agent。
- 从当天 `log.jsonl` 生成结构化总结。
- 用同一套 Summary Agent 结果驱动主页面右侧每半小时摘要。
- 保持日报本地文件输出为 `summary.md`。
- 总结内容要温和、证据导向，避免评判用户。
- Agent 不直接读写任意文件，只接收 Rust 传入的当天日志 JSON。

## 非目标

- 不做多 Agent 编排。
- 不做长期记忆、向量库或跨天检索。
- 不做聊天式总结入口。
- 不让 Agent 控制截图、录制、文件写入或系统能力。
- 不把本地视觉识别路径迁移到云端。

## SDK 选择

使用 Vercel AI SDK v6，具体采用 `ai` 包中的 `ToolLoopAgent`，并配合 `@ai-sdk/deepseek` 和 `zod`。

选择理由：

- 当前项目是 Vite/React/Bun/Tauri，AI SDK 与 TypeScript/Bun 工具链匹配。
- `ToolLoopAgent` 可以把模型、指令、工具和停止条件封装成可复用组件。
- AI SDK 支持结构化输出，适合将日报总结约束为稳定 JSON。
- MVP 阶段固定接入 DeepSeek API；未来如果要替换 AI Gateway 或其他 provider，AI SDK 的 provider 模型比直接写单一 API 更容易扩展。

控制点：

- 第一版只定义一个 Summary Agent。
- Agent 最多允许少量步骤，默认不需要循环调用工具。
- Agent 输入由 Rust 聚合后传入，不开放文件系统工具。
- Rust 负责 schema 之外的业务校验和 Markdown 渲染。

## 架构

```text
React UI
  ├─ getTodaySummaryBlocks()
  │   └─ Tauri command: get_today_summary_blocks
  │       └─ Rust SummaryService
  │           ├─ read today log.jsonl
  │           ├─ group entries into 30-minute buckets
  │           ├─ read cached SummaryTimeBlock records
  │           ├─ optionally summarize only the previous completed half-hour
  │           └─ return SummaryTimeBlock[]
  └─ generateSummary()
      └─ Tauri command: generate_summary
          └─ Rust SummaryService
              ├─ fill missing completed half-hour summaries
              ├─ call Bun sidecar for daily rollup
              ├─ parse SummaryAgentResult JSON
              └─ write summary.md

Bun sidecar
  └─ Vercel AI SDK ToolLoopAgent
      ├─ model provider
      ├─ summary instructions
      ├─ optional local summarization tools
      └─ structured JSON result
```

## 数据流

1. 用户打开主页面或记录刷新。
2. Rust 读取当天 `log.jsonl`，并读取已缓存的半小时摘要。
3. 如果本地时间已经跨过半小时边界，Rust 只检查刚结束的前半小时窗口。
4. 如果这个窗口有记录且尚未总结，Rust 将该窗口内的记录裁剪为 Summary Agent 输入：
   - 时间戳。
   - 今日项目。
   - `intent`。
   - `is_on_project`。
   - `confidence`。
   - `reason`。
   - `visible_context`。
   - `error`。
5. Rust 调用 `bun workers/summary_agent.ts`，通过 stdin 传入前半小时窗口 JSON。
6. Summary Agent 返回该窗口的结构化摘要，Rust 校验后写入缓存。
7. 主页面读取已完成半小时窗口的摘要缓存，渲染右侧每半小时摘要。
8. 用户生成日报时，Rust 补齐缺失窗口，并将当天全部窗口摘要和必要的原始日志一起交给 Summary Agent 汇总，再渲染为 `summary.md`。
9. UI 仍返回生成后的 Markdown 文件路径。

## Summary Agent 输入

```json
{
  "date": "2026-04-28",
  "project": "MindBack 总结 Agent",
  "entries": [
    {
      "timestamp": "2026-04-28T09:30:00+08:00",
      "intent": "正在实现日报生成",
      "is_on_project": true,
      "confidence": 0.86,
      "reason": "截图显示正在编辑 MindBack 相关文件。",
      "visible_context": "编辑器中打开 summary_agent.ts",
      "error": null
    }
  ]
}
```

## Summary Agent 输出

```json
{
  "overview": "今天主要围绕 MindBack 总结 Agent 设计和实现展开。",
  "projectAlignment": {
    "onProjectRatio": 82,
    "assessment": "focused"
  },
  "timeBlocks": [
    {
      "start": "09:30",
      "end": "10:00",
      "status": "on_project",
      "summary": "集中处理总结 Agent 的接口设计。",
      "evidence": ["多条记录显示正在编辑 MindBack 相关文档和代码。"]
    }
  ],
  "notableDrifts": [
    {
      "time": "14:10",
      "reason": "记录显示可见内容与今日项目关联较弱。"
    }
  ],
  "reflectionPrompts": [
    "哪些时间段最容易偏离今日项目？",
    "明天是否需要把高风险时间段提前安排为低干扰任务？"
  ]
}
```

枚举约束：

- `assessment`: `focused`、`mixed`、`drifted`、`insufficient_data`。
- `timeBlocks.status`: `on_project`、`off_project`、`uncertain`。

## Markdown 日报格式

Rust 根据 Agent JSON 渲染 Markdown，避免模型输出任意格式。

日报包含：

- 日期和今日项目。
- 记录数量、符合项目比例、整体判断。
- 总览。
- 时间段摘要。
- 明显偏离或不确定片段。
- 温和复盘问题。
- 数据来源说明。

日报不包含：

- 心理诊断。
- 训诫式语言。
- 对用户人格或能力的评价。
- 未在日志中出现的事实推断。

## 主页面右侧半小时摘要

主页面右侧当前的“时间段摘要”按 30 分钟窗口展示当天活动概览。这个区域使用 Summary Agent 的增量窗口摘要，而不是每次刷新都重新总结全天记录，也不是继续只在前端用 dominant intent 和符合比例拼接文本。

行为要求：

- 每个已结束的半小时窗口对应一个 `timeBlocks` 条目。
- Summary Agent 只总结刚结束的前半小时窗口。例如当前时间进入 10:00 后，才总结 09:30-10:00。
- 当前正在进行的半小时窗口继续使用本地规则摘要或显示“记录中”，不调用模型生成正式摘要。
- `timeBlocks.start` 和 `timeBlocks.end` 使用本地时间 `HH:mm`。
- `timeBlocks.status` 控制卡片状态：符合、偏离、不确定。
- `timeBlocks.summary` 作为卡片主标题或主要描述。
- `timeBlocks.evidence` 用于卡片次级说明，最多展示 1-2 条。
- 当 Summary Agent 尚未返回或调用失败时，UI 使用现有本地规则摘要作为回退。

刷新策略：

- 记录列表变化后，不对每条记录立即调用云端模型。
- 当本地时间跨过半小时边界时，Rust 检查刚结束窗口是否已经有摘要。
- 如果该窗口有记录且没有摘要，则调用 Summary Agent 一次。
- 如果该窗口无记录，写入 `insufficient_data` 摘要，不调用模型。
- Rust 按窗口缓存 `SummaryTimeBlock`，避免主页面刷新导致重复调用模型。
- 用户点击生成日报时，不重新生成每个半小时摘要；只补齐缺失窗口，再生成全天日报。

窗口输入：

- 该半小时内的 `LogEntry` 文本字段，也就是每张截图已有的识别 summary。
- 该半小时内可选的缩略图引用。第一版默认仍传文本化图片识别结果；后续如果需要让 Summary Agent 直接看图，只允许传这个窗口内的缩略图，不传全天图片。

接口建议：

- `get_today_summary_blocks() -> Vec<SummaryTimeBlock>`：返回右侧半小时摘要，优先读取缓存；无缓存时使用本地规则摘要。
- `summarize_previous_half_hour() -> Option<SummaryTimeBlock>`：只总结刚结束的前半小时窗口，更新缓存。
- `generate_summary() -> String`：补齐缺失窗口后，把窗口摘要汇总成 `summary.md`。

## 配置

第一版新增最小配置：

- `summary_model`: 默认 `deepseek-chat`。
- `summary_provider`: 默认 `deepseek`。
- `summary_enabled`: 默认开启。

环境变量：

- `DEEPSEEK_API_KEY`: Summary Agent 调用 DeepSeek provider 时需要。
- `MINDBACK_SUMMARY_AGENT_COMMAND`: 可选，覆盖默认 Bun 命令。
- `MINDBACK_SUMMARY_AGENT_PATH`: 可选，覆盖默认 sidecar 路径。

如果缺少 API key 或 sidecar 启动失败，Rust 回退到当前规则版 Markdown 日报，并在日报末尾写明总结 Agent 不可用。

## 文件变更计划

- `package.json`: 增加 `ai`、`@ai-sdk/deepseek`、`zod`。
- `workers/summary_agent.ts`: 新增 Summary Agent sidecar。
- `src-tauri/src/summary.rs`: 新增总结 Agent 调用、JSON 校验、前半小时窗口摘要缓存和 Markdown 渲染。
- `src-tauri/src/storage.rs`: 将现有 `write_today_summary` 委托给 summary 服务。
- `src-tauri/src/models.rs`: 增加 summary 配置字段和结构化结果类型。
- `src-tauri/src/commands.rs`: 保持 `generate_summary` 命令外部行为不变，并新增半小时摘要读取/前半小时总结命令。
- `src/lib/api.ts`: 增加半小时摘要 API wrapper。
- `src/lib/types.ts`: 增加 Summary Agent 输出和半小时摘要类型。
- `src/App.tsx`: 右侧“时间段摘要”优先展示 Summary Agent 结果，失败时使用当前本地规则摘要。
- `docs/implementation-notes.md`: 更新总结 Agent 的限制和运行要求。

## 错误处理

- 当当天无记录时，直接生成 `insufficient_data` 日报，不调用模型。
- 当 Agent 输出 JSON 解析失败时，写入错误说明，并回退到规则总结。
- 当模型调用失败时，保留原始日志不变，日报生成仍应成功。
- 当单条日志包含错误事件时，日报中按“不确定”处理，不把错误事件当成用户偏离。
- 当右侧半小时摘要刷新失败时，UI 不显示错误弹窗，继续展示本地规则摘要，并在设置或状态信息中保留最近错误。

## 测试策略

- Rust 单元测试：
  - 空日志生成 `insufficient_data`。
  - mock sidecar 返回合法 JSON 时生成 Markdown。
  - mock sidecar 返回非法 JSON 时回退规则总结。
  - sidecar 启动失败时回退规则总结。
  - 半小时摘要按本地时区稳定分桶。
  - 缓存存在时 `get_today_summary_blocks` 不重复调用 sidecar。
  - 只有刚结束且未总结的半小时窗口会触发 sidecar。
- TypeScript 测试或 dry run：
  - `summary_agent.ts` 能从 stdin 读取输入。
  - 缺少 `DEEPSEEK_API_KEY` 时返回结构化错误。
  - schema 能拒绝非法枚举值。
- 集成验证：
  - `bun run build`。
  - `cargo test --manifest-path src-tauri/Cargo.toml summary`。
  - 手动调用 `generate_summary`，确认 `summary.md` 可读。
  - 主页面右侧半小时摘要在 Agent 成功、Agent 失败、无记录三种状态下都可读。

## 隐私和安全

Summary Agent 第一版只接收文本化日志，不接收截图图片。日志中仍可能包含窗口标题、文档名或可见上下文，因此：

- 不上传缩略图。
- 不上传原始截图。
- 不上传高分辨率识别输入图。
- 不提供任意文件读取工具。
- 日报中保留“由 AI 总结生成，基于本地监督日志”的说明。

## 后续扩展

- 增加本地 summary provider，复用 MLX 文本/视觉模型。
- 增加用户可选的云端 provider 设置。
- 增加跨天趋势，但只基于用户明确选择的日期范围。
- 增加日报重新生成按钮，保留旧版本备份。
