import { z } from "zod";

const SummaryBlockStatusSchema = z.enum([
  "on_project",
  "off_project",
  "uncertain",
  "insufficient_data",
]);

const SummaryAssessmentSchema = z.enum([
  "focused",
  "mixed",
  "drifted",
  "insufficient_data",
]);

const SummaryLogEntrySchema = z.object({
  timestamp: z.string(),
  intent: z.string(),
  isOnProject: z.boolean(),
  confidence: z.number(),
  reason: z.string(),
  visibleContext: z.string(),
  error: z.string().nullable(),
});

const SummaryTimeBlockSchema = z.object({
  start: z.string(),
  end: z.string(),
  status: SummaryBlockStatusSchema,
  summary: z.string(),
  evidence: z.array(z.string()),
  recordCount: z.number().int().nonnegative(),
  onProjectRatio: z.number().int().min(0).max(100),
  error: z.string().nullable(),
});

const SummaryAgentResultSchema = z.object({
  overview: z.string(),
  projectAlignment: z.object({
    onProjectRatio: z.number().int().min(0).max(100),
    assessment: SummaryAssessmentSchema,
  }),
  timeBlocks: z.array(SummaryTimeBlockSchema),
  notableDrifts: z.array(
    z.object({
      time: z.string(),
      reason: z.string(),
    }),
  ),
  reflectionPrompts: z.array(z.string()),
  error: z.string().nullable(),
});

const SummaryAgentRequestSchema = z.object({
  task: z.enum(["window", "daily"]),
  date: z.string(),
  project: z.string(),
  windowStart: z.string().nullable(),
  windowEnd: z.string().nullable(),
  entries: z.array(SummaryLogEntrySchema),
  timeBlocks: z.array(SummaryTimeBlockSchema),
});

type SummaryAgentResult = z.infer<typeof SummaryAgentResultSchema>;
type SummaryAgentRequest = z.infer<typeof SummaryAgentRequestSchema>;

async function readStdin() {
  return await new Response(Bun.stdin.stream()).text();
}

function emptyResult(error: string): SummaryAgentResult {
  return {
    overview: "Summary Agent 不可用，已回退到本地摘要。",
    projectAlignment: {
      onProjectRatio: 0,
      assessment: "insufficient_data",
    },
    timeBlocks: [],
    notableDrifts: [],
    reflectionPrompts: [],
    error,
  };
}

function localWindowFallback(request: SummaryAgentRequest): SummaryAgentResult {
  const count = request.entries.length;
  const onProject = request.entries.filter((entry) => entry.isOnProject).length;
  const ratio = count === 0 ? 0 : Math.round((onProject / count) * 100);
  const summary =
    request.entries[0]?.intent ?? "该时间段没有可用于总结的记录。";
  const status =
    count === 0
      ? "insufficient_data"
      : ratio >= 60
        ? "on_project"
        : ratio === 0
          ? "off_project"
          : "uncertain";

  return {
    overview: count === 0 ? "该时间段没有记录。" : summary,
    projectAlignment: {
      onProjectRatio: ratio,
      assessment:
        count === 0
          ? "insufficient_data"
          : ratio >= 70
            ? "focused"
            : ratio >= 40
              ? "mixed"
              : "drifted",
    },
    timeBlocks: [
      {
        start: request.windowStart ?? "",
        end: request.windowEnd ?? "",
        status,
        summary,
        evidence: request.entries
          .flatMap((entry) => [entry.visibleContext, entry.reason])
          .filter(Boolean)
          .slice(0, 2),
        recordCount: count,
        onProjectRatio: ratio,
        error: null,
      },
    ],
    notableDrifts: [],
    reflectionPrompts: [],
    error: null,
  };
}

function buildPrompt(request: SummaryAgentRequest) {
  const taskInstruction =
    request.task === "window"
      ? [
          "总结这个刚结束的半小时窗口。必须返回 exactly one timeBlocks item。",
          "窗口摘要应优先概括用户在这一段时间主要关注的应用、内容和任务。",
          "status 必须主要依据 entries 的 isOnProject、confidence 和 reason，不要因为措辞像工作就自行改判。",
          "summary 控制在一句中文内；evidence 选 1 到 3 条来自输入的可见证据，优先保留活跃应用名、窗口标题、页面/文件名、正在处理的主题。",
        ].join(" ")
      : [
          "基于半小时窗口摘要生成当天日报总览。",
          "日报应聚合用户在哪些应用和内容之间切换、哪些时间段最贴近今日项目、哪些片段明显偏离或证据不足。",
          "不要重新分析原始截图，不要编造没有出现在 entries 或 timeBlocks 中的应用、网页、文件、任务或动机。",
          "timeBlocks 必须返回空数组；宿主程序会复用输入中的缓存时间段摘要。",
          "reflectionPrompts 给 2 到 4 个具体复盘问题，问题应围绕时间安排、上下文切换和今日项目推进，不做心理诊断。",
        ].join(" ");

  return JSON.stringify(
    {
      instruction:
        "你是 MindBack 的总结 Agent。只基于输入记录总结，不做心理诊断，不训诫用户，不编造事实。输入中的 reason 和 visibleContext 通常已经包含活跃应用、应用用途、窗口标题、页面/文件名和可见内容；这些是判断用户正在关注什么的主要证据。输出必须温和、事实化、证据导向、可复盘。只返回 JSON，不要返回 Markdown 或额外解释。",
      outputContract: {
        overview: "string",
        projectAlignment: {
          onProjectRatio: "integer 0-100",
          assessment: "focused | mixed | drifted | insufficient_data",
        },
        timeBlocks:
          "array of { start, end, status, summary, evidence, recordCount, onProjectRatio, error }",
        notableDrifts: "array of { time, reason }",
        reflectionPrompts: "string[]",
        error: null,
      },
      task: taskInstruction,
      request,
    },
    null,
    2,
  );
}

async function runAgent(request: SummaryAgentRequest): Promise<SummaryAgentResult> {
  const apiKey = process.env.DEEPSEEK_API_KEY;
  if (!apiKey) {
    throw new Error("DEEPSEEK_API_KEY is not set");
  }

  const model = process.env.MINDBACK_SUMMARY_MODEL ?? "deepseek-v4-flash";
  const baseUrl = process.env.DEEPSEEK_BASE_URL ?? "https://api.deepseek.com";
  const response = await fetch(`${baseUrl}/chat/completions`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${apiKey}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      model,
      messages: [
        {
          role: "system",
          content:
            "You summarize local productivity records for MindBack. Use only provided data. Return compact JSON only.",
        },
        { role: "user", content: buildPrompt(request) },
      ],
      response_format: { type: "json_object" },
      thinking: { type: "disabled" },
      max_tokens: 4096,
      temperature: 0.2,
      stream: false,
    }),
    signal: AbortSignal.timeout(60_000),
  });

  const responseText = await response.text();
  if (!response.ok) {
    throw new Error(`DeepSeek API ${response.status}: ${responseText}`);
  }

  const completion = JSON.parse(responseText) as {
    choices?: Array<{ message?: { content?: unknown } }>;
  };
  const content = completion.choices?.[0]?.message?.content;
  if (typeof content !== "string") {
    throw new Error("DeepSeek response did not include message content");
  }

  const output = JSON.parse(content);
  const parsed = SummaryAgentResultSchema.safeParse(output);
  if (!parsed.success) {
    throw new Error(parsed.error.message);
  }
  return parsed.data;
}

async function main() {
  try {
    const input = await readStdin();
    const request = SummaryAgentRequestSchema.parse(JSON.parse(input));

    if (request.task === "window" && request.entries.length === 0) {
      console.log(JSON.stringify(localWindowFallback(request)));
      return;
    }

    console.log(JSON.stringify(await runAgent(request)));
  } catch (error) {
    console.log(JSON.stringify(emptyResult(String(error))));
  }
}

await main();
