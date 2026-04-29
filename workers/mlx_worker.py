#!/usr/bin/env python3
"""MindBack MLX-VLM worker contract.

The worker always prints one JSON object. If mlx-vlm is not installed or the
image cannot be read, it returns a structured error instead of a traceback.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any


def emit(payload: dict[str, Any]) -> None:
    print(json.dumps(payload, ensure_ascii=False))


def unavailable(reason: str) -> int:
    emit(
        {
            "error": "worker_unavailable",
            "intent": "未能完成本地视觉识别",
            "is_on_project": False,
            "confidence": 0.0,
            "reason": reason,
            "visible_context": "",
        }
    )
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="运行 MindBack MLX-VLM 本地识别")
    parser.add_argument("--model", required=True)
    parser.add_argument("--project", required=True)
    parser.add_argument("--image", required=True)
    return parser.parse_args()


def parse_model_json(text: str) -> dict[str, Any]:
    text = text.strip()
    if text.startswith("```"):
        text = re.sub(r"^```(?:json)?\s*", "", text)
        text = re.sub(r"\s*```$", "", text)

    try:
        return json.loads(text)
    except json.JSONDecodeError:
        match = re.search(r"\{.*\}", text, flags=re.DOTALL)
        if match:
            return json.loads(match.group(0))
        raise


def main() -> int:
    args = parse_args()
    image_path = Path(args.image).expanduser()
    if not image_path.exists():
        return unavailable(f"图片不存在：{image_path}")

    try:
        from mlx_vlm import apply_chat_template, generate, load  # type: ignore
    except Exception as exc:  # pragma: no cover - depends on local MLX install
        return unavailable(f"mlx-vlm 不可用：{exc}")

    prompt = (
        "你是 MindBack 的本地监督日志识别器。请只输出 JSON，不要输出解释或 Markdown。\n"
        f"今日项目：{args.project}\n"
        "先观察截图左上角 macOS 菜单栏显示的前台活跃应用名称；这是判断用户当前主要行为的优先信号。"
        "如果能识别应用，请结合该应用通常用途判断用户大概在做什么，再结合窗口标题、页面正文、代码、文档、聊天内容或浏览器标签判断用户正在关注的具体内容。"
        "如果左上角应用名不可读，再使用屏幕中最显著的窗口内容推断。\n"
        "判断是否符合今日项目时，只基于截图中可见证据；不确定时不要强行判定为符合项目。\n"
        "必须返回字段：intent, is_on_project, confidence, reason, visible_context。\n"
        "字段要求：intent 用一句话概括用户正在做的事；is_on_project 表示该行为是否直接服务今日项目；"
        "confidence 是 0 到 1 的数字，0.8 以上仅用于活跃应用和内容都清晰且与项目关系明确的情况，0.4 到 0.7 用于部分证据或间接相关，0.4 以下用于看不清或无法判断；"
        "reason 说明你的判断依据，优先提到活跃应用、应用用途和可见内容如何对应今日项目；"
        "visible_context 记录屏幕上可见的关键证据，例如活跃应用名、窗口标题、页面/文件名、正在编辑或阅读的主题。\n"
        "所有字段值必须使用简体中文，除非截图中出现必须保留的代码、命令、文件名或专有名词。"
    )

    try:
        model, processor = load(args.model)
        formatted_prompt = apply_chat_template(
            processor,
            model.config,
            prompt,
            num_images=1,
        )
        response = generate(
            model,
            processor,
            prompt=formatted_prompt,
            image=[str(image_path)],
            max_tokens=256,
            temperature=0.0,
        )
    except Exception as exc:  # pragma: no cover - depends on local MLX/model state
        return unavailable(f"mlx-vlm 识别失败：{exc}")

    text = getattr(response, "text", str(response)).strip()
    try:
        payload = parse_model_json(text)
    except json.JSONDecodeError:
        emit(
            {
                "error": "parse_error",
                "intent": "模型返回了非 JSON 内容",
                "is_on_project": False,
                "confidence": 0.0,
                "reason": text[:500],
                "visible_context": "",
            }
        )
        return 0

    payload.setdefault("error", None)
    emit(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
