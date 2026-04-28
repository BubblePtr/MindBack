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
        "你是 MindBack 的本地监督日志识别器。"
        "请判断截图中的当前行为是否符合今日项目，并只输出 JSON。"
        f"今日项目：{args.project}\n"
        "字段必须包含 intent, is_on_project, confidence, reason, visible_context。"
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
