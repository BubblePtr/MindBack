import { deepseek } from "@ai-sdk/deepseek";
import { Output, ToolLoopAgent, stepCountIs } from "ai";
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
  return JSON.stringify(
    {
      instruction:
        "你是 MindBack 的总结 Agent。只基于输入记录总结，不做心理诊断，不训诫用户，不编造事实。输出必须温和、证据导向、可复盘。",
      task:
        request.task === "window"
          ? "总结这个刚结束的半小时窗口。必须返回 exactly one timeBlocks item。"
          : "基于半小时窗口摘要生成当天日报总览。",
      request,
    },
    null,
    2,
  );
}

async function runAgent(request: SummaryAgentRequest): Promise<SummaryAgentResult> {
  const model = process.env.MINDBACK_SUMMARY_MODEL ?? "deepseek-chat";
  const agent = new ToolLoopAgent({
    model: deepseek(model),
    instructions:
      "You summarize local productivity records for MindBack. Use only provided data. Keep the tone calm, factual, and non-judgmental.",
    output: Output.object({ schema: SummaryAgentResultSchema }),
    stopWhen: stepCountIs(1),
    temperature: 0.2,
  });

  const result = await agent.generate({
    prompt: buildPrompt(request),
    timeout: { totalMs: 60_000 },
  });
  const output = (result as { output?: unknown }).output;
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

    if (!process.env.DEEPSEEK_API_KEY) {
      console.log(JSON.stringify(emptyResult("DEEPSEEK_API_KEY is not set")));
      return;
    }

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
